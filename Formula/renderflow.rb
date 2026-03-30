# typed: false
# frozen_string_literal: true

class Renderflow < Formula
  desc "Spec-driven document rendering engine"
  homepage "https://github.com/egohygiene/renderflow"
  # url and sha256 are updated automatically by CI on each tagged release.
  # Until the first release is published, install via: brew install --HEAD renderflow
  url "https://github.com/egohygiene/renderflow/archive/refs/tags/v0.1.0.tar.gz"
  sha256 "0000000000000000000000000000000000000000000000000000000000000000"
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
