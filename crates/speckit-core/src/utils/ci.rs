use std::env;

/// Check if running in a CI environment.
pub fn is_ci_environment() -> bool {
    // Check common CI environment variables
    env::var("CI").is_ok()
        || env::var("CONTINUOUS_INTEGRATION").is_ok()
        || env::var("GITHUB_ACTIONS").is_ok()
        || env::var("TRAVIS").is_ok()
        || env::var("CIRCLECI").is_ok()
        || env::var("JENKINS_URL").is_ok()
        || env::var("GITLAB_CI").is_ok()
        || env::var("BITBUCKET_PIPELINE").is_ok()
        || env::var("BUILDKITE").is_ok()
        || env::var("CODEBUILD_BUILD_ID").is_ok()
        || env::var("TF_BUILD").is_ok() // Azure Pipelines
        || env::var("TEAMCITY_VERSION").is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_ci_environment_default() {
        // In test environment, CI should be false unless set
        // This test verifies the function doesn't panic
        let _ = is_ci_environment();
    }
}
