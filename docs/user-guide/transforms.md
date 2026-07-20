# Transforms

        Transforms are in-memory document mutations that run before rendering or, in graph mode, act as edges between formats.

        ## Built-in document transforms

        | Transform | What it does | Source |
        |---|---|---|
        | `EmojiTransform` | Replaces emoji with `[emoji]` for non-HTML targets | `src/transforms/emoji.rs` |
        | `VariableSubstitutionTransform` | Replaces `{{key}}` placeholders from `variables` | `src/transforms/variable.rs` |
        | `SyntaxHighlightTransform` | Normalizes fenced-code language tags to lowercase | `src/transforms/syntax_highlight.rs` |

        ## Order of execution

        The standard registry always registers built-ins in this order:

        1. emoji
        2. variables
        3. syntax-highlight normalization

        That order is defined in `register_transforms`.

        ## Command transforms

        YAML-defined command transforms are loaded from `transforms:` files and can run either:

        - **stdin/stdout mode** when no `{input}` / `{output}` placeholders exist
        - **file mode** when placeholders are present

        Example:

        ```yaml
        transforms:
          - name: md-to-html
            program: pandoc
            args: ["{input}", "-o", "{output}"]
            from: markdown
            to: html
            cost: 0.5
            quality: 0.95
        ```

        ## AI transforms

        AI transforms add fields such as `ai`, `model`, `prompt`, `endpoint`, `api_key_env`, `cache_path`, and `prompt_version`.

        ```yaml
        transforms:
          - name: summarize
            ai: ollama
            model: mistral
            prompt: "Summarize this document:

{input}"
            from: markdown
            to: markdown
            cost: 2.0
            quality: 0.8
        ```

        ## Aggregation transforms

        Setting `input_kind: collection` marks a transform as an aggregation edge. Those transforms combine multiple inputs into one output and are registered in an `AggregationRegistry`.

        ## Failure modes

        `TransformRegistry` supports:

        - `FailFast` — stop on the first error
        - `ContinueOnError` — log the failure and pass the unmodified input to the next transform

        !!! warning
            A transform YAML entry must supply exactly one of `program`, `plugin`, or `ai` from a schema perspective. In practice, graph execution currently materializes command and AI transforms directly; plugin-backed graph edges are not yet wired by the graph builder.
