## Summary
Brief description of the changes in this PR.

## Type of Change
- [ ] Bug fix (non-breaking change which fixes an issue)
- [ ] New feature (non-breaking change which adds functionality)
- [ ] Breaking change (fix or feature that would cause existing functionality to not work as expected)
- [ ] Documentation update
- [ ] Performance improvement
- [ ] Code refactoring

## Changes Made
- List the main changes
- Use bullet points
- Be specific about what was modified

## Testing
- [ ] Tests pass locally with `cargo test`
- [ ] Code follows the project's style guidelines (`cargo fmt` and `cargo clippy`)
- [ ] Self-review of the code has been performed
- [ ] Code has been tested manually (if applicable)

### Test Environment
- OS: [e.g. Arch Linux]
- Wayland Compositor: [e.g. Hyprland]
- Audio System: [e.g. PipeWire]

## Audio Feedback Testing (if applicable)
- [ ] Recording start beep plays correctly
- [ ] Recording stop beep plays correctly  
- [ ] Success beep plays after operations
- [ ] Error beep plays on failures
- [ ] Volume controls work as expected

## Signal Testing (if applicable)
- [ ] SIGUSR1 (direct typing) works correctly
- [ ] SIGUSR2 (clipboard copy) works correctly
- [ ] Error handling works properly

## Breaking Changes
If this PR contains breaking changes, please describe:
- What breaks?
- How to migrate existing usage?
- Updated documentation?

## Additional Notes
Any additional information, context, or screenshots that would be helpful for reviewers.

## Checklist
- [ ] My code follows the style guidelines of this project
- [ ] I have performed a self-review of my own code
- [ ] I have commented my code, particularly in hard-to-understand areas
- [ ] I have made corresponding changes to the documentation
- [ ] My changes generate no new warnings
- [ ] I have added tests that prove my fix is effective or that my feature works
- [ ] New and existing unit tests pass locally with my changes