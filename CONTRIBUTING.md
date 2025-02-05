# Contribute to `phantomlink`

We welcome any kind of contribution. Do not hesitate to contact us if you have any doubts, and feel free to open an issue or a pull request.
If you have ideas that affect the overall architecture of the tool or the behavior of already existing features, please open a new [discussion](https://github.com/robinohs/phantomlink/discussions/new/choose) first.

Even though we are not directly affiliated, this project follows the [Rust Code of Conduct](https://www.rust-lang.org/policies/code-of-conduct) and expects all contributors to do so.

## Guidelines

In order to create and maintain a certain level of code quality, this project follows a set of coding guidelines.

### Code Quality Assurance

Any code pushed to the repository should be:

- well-formatted:

  ```sh
  cargo fmt --all
  ```

- tested and passing to prevent bugs:

  ```sh
  cargo test
  ```

- not throwing any clippy errors to provide optimized code:

  ```sh
  cargo clippy
  ```

### Git Commits

Git commit messages should apply to the following rules (inspired by [joshbuchea](https://gist.github.com/joshbuchea/6f47e86d2510bce28f8e7f42ae84c716)):

- Format: `<type>(<scope>)!: <subject>`, while `<scope>` is optional and ! is only used if it is a breaking change
- the list of types is:
  - feat: (new feature for the user, not a new feature for build script)
  - fix: (bug fix for the user, not a fix to a build script)
  - docs: (changes to the documentation)
  - style: (formatting, missing semi colons, etc; no production code change)
  - refactor: (refactoring production code, eg. renaming a variable)
  - test: (adding missing tests, refactoring tests; no production code change)
  - chore: (updating grunt tasks etc; no production code change)
  - CI: (update the pipeline; no production code change)
  - build: (update dependencies or building infrastructure that has an influence on production code)
  - bench: (changes to benchmarking scripts or code; no production code change)
  - rm: (remove a feature)
- commits should not contain several different commit types (e.g., change build scripts and production code at the same time), but should be specific and traceable
- Example:

    ```c
    feat: add hat wobble
    ^--^  ^------------^
    |     |
    |     +-> Summary in present tense.
    |
    +-------> Type: feat, fix, docs, style, refactor, test, chore, build
    ```
