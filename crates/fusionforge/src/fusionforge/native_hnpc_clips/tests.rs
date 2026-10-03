use super::*;
#[test]
fn explicit_clip_names_are_unique_identifiers() {
    assert!(names("stand2,talk").is_ok());
    for invalid in ["stand2,stand2", "../stand2", "", "talk,"] {
        assert!(names(invalid).is_err());
    }
}
#[test]
fn playback_retains_accepted_loop_and_clamp_rules() {
    for n in ["stun", "rifledash", "rocketjumpstart", "woundupper"] {
        assert_eq!(playback(n), "clamp");
    }
    for n in ["talk", "stand2", "rifleguard"] {
        assert_eq!(playback(n), "loop");
    }
}
