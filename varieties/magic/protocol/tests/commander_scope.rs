use cardbench_magic_protocol::{FormatCapability, ProtocolCapabilities, SupportLevel};

#[test]
fn mini_commander_is_not_full_edh() {
    let capabilities = ProtocolCapabilities::current();
    assert_eq!(
        capabilities.format(FormatCapability::Commander),
        SupportLevel::Supported
    );
    assert_eq!(
        capabilities.format(FormatCapability::CommanderEdh),
        SupportLevel::Absent
    );
}
