# 📦 Release Checklist

## Pre-Release

- [ ] All tests passing (`cargo test`)
- [ ] Clippy warnings resolved (`cargo clippy`)
- [ ] Code formatted (`cargo fmt`)
- [ ] Security audit clean (`cargo audit`)
- [ ] Documentation updated
- [ ] CHANGELOG.md updated
- [ ] Version bumped in Cargo.toml

## Security

- [ ] No critical vulnerabilities in dependencies
- [ ] Security fixes documented in SECURITY.md
- [ ] Penetration test completed (if applicable)
- [ ] Security score >= 8.0/10

## Release Steps

1. Create release branch: `git checkout -b release/v0.1.0`
2. Update version in Cargo.toml
3. Update CHANGELOG.md
4. Create PR and merge
5. Create git tag: `git tag -a v0.1.0 -m "Release v0.1.0"`
6. Push tag: `git push origin v0.1.0`
7. Create GitHub release from tag
8. Publish to crates.io (if applicable)

## Post-Release

- [ ] GitHub release published
- [ ] Documentation deployed
- [ ] Announcement made (if applicable)
- [ ] Monitor for issues

---

**Template Version:** 1.0  
**Last Updated:** 2026-03-22
