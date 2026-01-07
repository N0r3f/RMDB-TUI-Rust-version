use std::process::Command;

#[derive(Debug, Clone, Default)]
pub struct Capabilities {
    pub has_systemctl: bool,
    pub has_service_cmd: bool,
    pub has_rc_service: bool,
    pub has_sv: bool,
    pub has_ip: bool,
    pub has_ss: bool,
    pub has_netstat: bool,
    pub has_lsof: bool,
    pub has_ping: bool,
    pub has_traceroute: bool,
    pub has_dig: bool,
    pub has_nslookup: bool,
    pub has_nft: bool,
    pub has_iptables: bool,
    pub has_ufw: bool,
    pub has_firewalld: bool,
    pub has_apt: bool,
    pub has_dnf: bool,
    pub has_yum: bool,
    pub has_pacman: bool,
    pub has_zypper: bool,
    pub has_rpm: bool,
    pub has_dpkg: bool,
    pub has_sudo: bool,
    // RMDB/IPXE specific
    pub has_rmdbd: bool,
    pub has_go: bool,
}

impl Capabilities {
    pub fn detect() -> Self {
        let has = |tool: &str| -> bool {
            Command::new("sh")
                .args(["-lc", &format!("command -v {} >/dev/null 2>&1", tool)])
                .status()
                .map(|s| s.success())
                .unwrap_or(false)
        };

        Self {
            has_systemctl: has("systemctl"),
            has_service_cmd: has("service"),
            has_rc_service: has("rc-service"),
            has_sv: has("sv"),
            has_ip: has("ip"),
            has_ss: has("ss"),
            has_netstat: has("netstat"),
            has_lsof: has("lsof"),
            has_ping: has("ping"),
            has_traceroute: has("traceroute"),
            has_dig: has("dig"),
            has_nslookup: has("nslookup"),
            has_nft: has("nft"),
            has_iptables: has("iptables"),
            has_ufw: has("ufw"),
            has_firewalld: has("firewall-cmd"),
            has_apt: has("apt"),
            has_dnf: has("dnf"),
            has_yum: has("yum"),
            has_pacman: has("pacman"),
            has_zypper: has("zypper"),
            has_rpm: has("rpm"),
            has_dpkg: has("dpkg"),
            has_sudo: has("sudo"),
            has_rmdbd: has("rmdbd"),
            has_go: has("go"),
        }
    }
}

