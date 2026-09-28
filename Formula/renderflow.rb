# typed: false
# frozen_string_literal: true

class Renderflow < Formula
  desc "Unpublished source-development formula for Renderflow"
  homepage "https://github.com/egohygiene/renderflow"
  # HEAD-only source-development template. No candidate Homebrew channel has
  # been published or independently verified. Do not add a stable URL until
  # its exact source archive and checksum have release evidence.
  license "MIT"
  head "https://github.com/egohygiene/renderflow.git", branch: "main"

  depends_on "rust" => :build
  depends_on "pandoc"

  def install
    system "cargo", "install", *std_cargo_args
  end

  test do
    assert_match "renderflow", shell_output("#{bin}/renderflow --version")
  end
end
