# Changelog

All notable changes to this project will be documented in this file.

## [0.1.0] - 2026-03-30

### Features

- Initialize Rust project with Cargo, CLI (clap), logging, and test setup
- Implement pipeline system for step-based execution
- Implement markdown → HTML pipeline step using pandoc
- Implement OutputStrategy trait for extensible rendering
- Implement HTML output strategy via pandoc (#39)
- Implement PdfStrategy using pandoc + tectonic (#40)
- Integrate output strategies into pipeline execution (#41)
- Automate GitHub Releases with compiled binaries on version tags (#67)
- Variable substitution transform ({{variable}} support) (#94)
- Syntax highlight pre-processing transform (V1) (#95)
- Implement modular transform registration system (#96)
- Add DOCX output strategy via pandoc (#98)
- *(transforms)* Named errors, fail-fast config, and transform lifecycle logging (#99)
- *(audits)* Generate advanced architecture audit with output strategy matrix and system evolution recommendations (#101)
- Introduce InputFormat abstraction with auto-detection (#128)
- Validate supported input/output format combinations (#130)
- Implement transform result caching using input hashing
- Implement output caching to skip redundant pandoc execution
- Implement watch mode for automatic rebuild on file changes
- Add cross compilation support using cross
- Add audit subcommand and generate optimization audit report
- Add research-paper Copilot agent for generation and polishing
- Add research paper template (Pandoc LaTeX + HTML compatible)"
- Add Debian package (.deb) support for APT distribution
- Create Arch Linux PKGBUILD for AUR (renderflow-git)
- Add RPM package support for YUM/DNF distribution
- Add Homebrew formula and macOS binary builds
- *(ci)* Integrate changelog generation into release workflow

### Fixes

- *(devcontainer)* Install Rust as vscode user to resolve cargo permission errors (#76)
- Replace to_string_lossy().into_owned() with to_str()? in strategies
- Replace unwrap in validate_input with proper error handling

### Improvements

- Update select_strategy to accept references instead of owned values


