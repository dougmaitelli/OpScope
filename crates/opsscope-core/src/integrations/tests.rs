use super::*;
use crate::application::SourceCapability;

#[test]
fn both_platforms_share_the_complete_source_catalog() {
    let registry = registered_sources().unwrap();
    for id in ["github", "gitlab", "gitea", "bitbucket"] {
        let descriptor = registry.get(id).unwrap().descriptor();
        assert_eq!(descriptor.id, id);
        assert!(descriptor.supports(SourceCapability::Workflows));
        assert!(descriptor.supports(SourceCapability::ChangeRequests));
        assert_eq!(
            descriptor.supports(SourceCapability::Issues),
            id != "bitbucket"
        );
    }
}
