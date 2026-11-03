-- renderflow.editorial/v1: validate article semantics on Pandoc's existing AST.
-- No shell commands, network discovery, content generation or external filters.
PANDOC_VERSION:must_be_at_least("3.6.1", "editorial.provider: Pandoc >= 3.6.1 is required")

local function require_value(ok, code)
  if not ok then error("editorial." .. code, 0) end
end

local function plain(value, field)
  require_value(type(value) == "string" or pandoc.utils.type(value) == "Inlines", "metadata." .. field .. ": expected text")
  local text = pandoc.utils.stringify(value)
  require_value(text:match("%S") ~= nil and #text <= 4096 and not text:match("[%z\1-\31\127]"), "metadata." .. field .. ": invalid text")
  return text
end

local function inline(value, field)
  return pandoc.MetaInlines({pandoc.Str(plain(value, field))})
end

local function text_list(value, field)
  require_value(pandoc.utils.type(value) == "List" and #value > 0 and #value <= 32, "metadata." .. field .. ": expected 1..32 strings")
  local result = pandoc.MetaList({})
  for _, item in ipairs(value) do result:insert(inline(item, field)) end
  return result
end

local function date(value, field)
  local text = plain(value, field)
  local year, month, day = text:match("^(%d%d%d%d)%-(%d%d)%-(%d%d)$")
  require_value(year ~= nil, "metadata." .. field .. ": expected YYYY-MM-DD")
  year, month, day = tonumber(year), tonumber(month), tonumber(day)
  local leap = year % 4 == 0 and (year % 100 ~= 0 or year % 400 == 0)
  local days = {31, leap and 29 or 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31}
  require_value(year > 0 and month >= 1 and month <= 12 and day >= 1 and day <= (days[month] or 0), "metadata." .. field .. ": invalid date")
  return pandoc.MetaString(text)
end

local allowed = {
  ["article-schema"] = true, title = true, subtitle = true, author = true,
  contributors = true, date = true, updated = true, ["canonical-url"] = true,
  keywords = true, lang = true, toc = true, hero = true,
}

local function metadata(meta)
  for key, _ in pairs(meta) do require_value(allowed[key], "metadata.unknown_field: " .. key) end
  require_value(meta["article-schema"] ~= nil and plain(meta["article-schema"], "article-schema") == "renderflow.editorial/v1", "metadata.schema: expected renderflow.editorial/v1")
  meta.title = inline(meta.title, "title")
  local lang = plain(meta.lang, "lang")
  require_value(#lang <= 64 and lang:match("^%a%a[%w%-]*$") and not lang:match("%-%-") and not lang:match("%-$"), "metadata.lang: expected a language tag")
  meta.lang = pandoc.MetaString(lang)
  if meta.subtitle then meta.subtitle = inline(meta.subtitle, "subtitle") end
  for _, field in ipairs({"author", "contributors", "keywords"}) do
    if meta[field] then meta[field] = text_list(meta[field], field) end
  end
  if meta.date then meta.date = date(meta.date, "date") end
  if meta.updated then meta.updated = date(meta.updated, "updated") end
  if meta.date and meta.updated then
    require_value(pandoc.utils.stringify(meta.updated) >= pandoc.utils.stringify(meta.date), "metadata.updated: precedes publication date")
  end
  if meta["canonical-url"] then
    local url = plain(meta["canonical-url"], "canonical-url")
    require_value(url:match("^https?://[^/]+") and not url:match("[%s<>\"'\\{}|^`]"), "metadata.canonical-url: expected an explicit HTTP(S) URL")
    meta["canonical-url"] = pandoc.MetaString(url)
    -- Pandoc's writer escapes MetaString through its inline writer. Do not
    -- pre-escape ampersands here or HTML would change the canonical URL.
  end
  require_value(meta.toc == nil or type(meta.toc) == "boolean", "metadata.toc: expected a boolean")
  return meta
end

local images = {}
local image_count = 0
local function image(element)
  local source = element.src
  require_value(#source <= 4096 and not source:find("%", 1, true) and not source:match("^[/\\]") and not source:match("^[%a][%w+.-]*:"),
    "image.path: use a source-relative local PNG or JPEG")
  for part in source:gmatch("[^/\\]+") do require_value(part ~= "..", "image.path: parent traversal is unsupported") end
  require_value(pandoc.utils.stringify(element.caption):match("%S") ~= nil, "image.alt: a description is required")
  local ext = source:lower():match("%.([a-z]+)$")
  require_value(ext == "png" or ext == "jpg" or ext == "jpeg", "image.format: only local PNG and JPEG are supported")
  if not images[source] then
    image_count = image_count + 1
    require_value(image_count <= 64, "image.limit: at most 64 distinct images")
    local ok, mime, bytes = pcall(pandoc.mediabag.fetch, source)
    require_value(ok and type(bytes) == "string" and #bytes > 0 and #bytes <= 16 * 1024 * 1024, "image.read: missing or oversized image")
    require_value(mime == "image/png" or mime == "image/jpeg", "image.media_type: expected PNG or JPEG")
    local name = "editorial-image-" .. image_count .. "." .. ext
    pandoc.mediabag.insert(name, mime, bytes)
    images[source] = name
  end
  element.src = images[source]
  element.attributes = {}
  return element
end

local function clean_attributes(element)
  -- IDs/classes are semantic hooks; arbitrary style/event attributes are outside
  -- this content contract. Code text and language classes remain untouched.
  require_value(not element.identifier:match("[%s<>\"'&]"), "content.identifier: unsupported characters")
  for _, class in ipairs(element.classes) do
    require_value(not class:match("[%s<>\"'&]"), "content.class: unsupported characters")
  end
  element.attributes = {}
  return element
end

local function link(element)
  local scheme = element.target:match("^([%a][%w+.-]*):")
  require_value(not scheme or scheme:lower() == "https" or scheme:lower() == "http" or scheme:lower() == "mailto", "link.scheme: unsupported link")
  return clean_attributes(element)
end

local function div(element)
  element = clean_attributes(element)
  if FORMAT == "latex" and (element.classes:includes("callout") or element.classes:includes("aside")) then
    local environment = element.classes:includes("aside") and "RenderflowAside" or "RenderflowCallout"
    element.content:insert(1, pandoc.RawBlock("latex", "\\begin{" .. environment .. "}"))
    element.content:insert(pandoc.RawBlock("latex", "\\end{" .. environment .. "}"))
  end
  return element
end

local function refuse_raw()
  error("editorial.content.raw: use Markdown semantics; raw HTML/TeX is unsupported", 0)
end

function Pandoc(doc)
  doc.meta = metadata(doc.meta)
  local hero = doc.meta.hero
  if hero then
    require_value(type(hero) == "table", "metadata.hero: expected path, alt and optional caption")
    for key, _ in pairs(hero) do require_value(key == "path" or key == "alt" or key == "caption", "metadata.hero: unknown field") end
    local picture = pandoc.Image({pandoc.Str(plain(hero.alt, "hero.alt"))}, plain(hero.path, "hero.path"))
    local caption = hero.caption and pandoc.Caption({pandoc.Plain({pandoc.Str(plain(hero.caption, "hero.caption"))})}) or pandoc.Caption({})
    doc.blocks:insert(1, pandoc.Figure({pandoc.Plain({picture})}, caption, pandoc.Attr("article-hero")))
    doc.meta.hero = nil
  end
  -- Walking only blocks avoids treating metadata as arbitrary executable markup.
  local body = pandoc.Pandoc(doc.blocks):walk({
    RawBlock = refuse_raw, RawInline = refuse_raw, Image = image, Link = link,
    Header = function(element)
      require_value(element.level >= 2, "content.heading: title is metadata; body headings start at level 2")
      return clean_attributes(element)
    end,
    CodeBlock = clean_attributes, Code = clean_attributes, Span = clean_attributes,
  })
  -- Add only our own LaTeX environments after refusing source-supplied raw markup.
  body = body:walk({Div = div})
  doc.blocks = body.blocks
  if FORMAT == "docx" or FORMAT:match("^epub") then
    for _, field in ipairs({"updated", "keywords", "contributors"}) do
      if doc.meta[field] then
        local values = doc.meta[field]
        local text = pandoc.utils.stringify(values)
        if pandoc.utils.type(values) == "List" then
          local parts = {}
          for _, value in ipairs(values) do table.insert(parts, pandoc.utils.stringify(value)) end
          text = table.concat(parts, "; ")
        end
        local label = ({updated = "Updated", keywords = "Topics", contributors = "Contributors"})[field]
        doc.blocks:insert(pandoc.Para({pandoc.Str(label .. ": " .. text)}))
      end
    end
  end
  if doc.meta["canonical-url"] and FORMAT ~= "html5" then
    local url = pandoc.utils.stringify(doc.meta["canonical-url"])
    doc.blocks:insert(pandoc.Para({pandoc.Str("Canonical URL: "), pandoc.Link({pandoc.Str(url)}, url)}))
  end
  return doc
end
