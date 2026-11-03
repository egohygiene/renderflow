# Prepare an editorial article

The repository agent `.github/agents/medium-article.agent.md` supports generation,
polishing, and partial-draft completion. Select **Editorial article** in a
Copilot environment that supports repository custom agents. Copilot availability
and model behavior depend on that environment; rendering never requires it.
The same instructions and Markdown contract can guide a manual author or another
assistant.

The agent prepares content for the [editorial profile](editorial-articles.md).
It does not render, research missing facts, or publish. It uses the existing
article schema rather than adding a parallel content model. The historical
filename's “Medium” reference describes a readable long-form workflow; the
profile uses Renderflow's own presentation and assets.

## Generate from notes

Example request (all events and names below are fictional):

> Write a short English article from these notes for neighbors. Title it
> “A shared shelf.” The fictional Cedar Lane group put a book shelf in its
> common room. Volunteers take turns tidying it. They want to leave a notebook
> for suggestions. Keep the conversational tone. No author or date is supplied.

An acceptable source-bound result is:

````markdown
---
article-schema: renderflow.editorial/v1
title: "A shared shelf"
lang: "en"
---
A book shelf now has a place in the Cedar Lane common room. Volunteers take
turns keeping it tidy.

## A place for suggestions

The group wants to leave a notebook beside the shelf so neighbors can suggest
what happens next.
````

The handoff can state that `en` follows the requested language. It must not add
participation counts, a launch date, attributed quotes, or an invented author.
A topic without these notes would not establish that the shelf or group exists.

## Polish an existing draft

Given this fictional input:

````markdown
---
title: "Keeping the shelf useful"
lang: "en"
---
# Keeping the shelf useful
We tidy the shelf each week. We tidy it so it stays usable.
Maybe the notebook will help, but we haven't tried it yet.
````

An acceptable normalized article is:

````markdown
---
article-schema: renderflow.editorial/v1
title: "Keeping the shelf useful"
lang: "en"
---
We tidy the shelf each week to keep it usable. The notebook may help, but we
haven't tried it yet.
````

The change note reports removal of repetition and the duplicate title heading,
and addition of the required schema. The uncertain notebook outcome stays
uncertain. No mandatory conclusion or extra sections are needed for this short
example. A longer article should use a hierarchy appropriate to its material.

## Complete or adapt a draft

Ask the same agent to finish a partial article, naming the paragraphs to retain
and supplying notes for the missing section. It should preserve existing claims
and citations, develop supported material, and list unresolved author questions
outside the document. It should flag unsupported images or raw HTML/TeX rather
than silently deleting them. Preserve the original before a substantial format
adaptation.

## Render separately

Save the reviewed article and local image assets in a project using the
[editorial example configuration](../../examples/editorial/renderflow.yaml).
The normal commands, when execution is in scope, are:

```bash
renderflow graph plan --config "renderflow.yaml"
renderflow build --config "renderflow.yaml"
```

Review actual published targets in the result. An agent's response is not a
successful render or approval to upload it. Images, fonts, providers, accessible
output, visual quality, and final content still need their applicable review.

## Configuration and current evidence

The agent follows GitHub's custom-agent YAML front-matter convention with a
required description and the `read`, `edit`, and `search` tool aliases. It has no
shell or web tool declaration. Model selection is left to the host. See
[GitHub's configuration reference](https://docs.github.com/en/copilot/reference/custom-agents-configuration).
The existing research-paper agent remains separately scoped to research papers.

The examples are authored expected behavior, not observed model evaluations.
No Copilot session, render, tests, build, documentation validation, or CI has
run for this implementation. #378 retains that acceptance work; #379 retains
renderer qualification and #458/#419 retain baseline/release ownership.
