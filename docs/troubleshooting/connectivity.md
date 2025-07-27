# Connectivity and Network Troubleshooting

> Diagnose and resolve network connectivity issues with Karere and WhatsApp Web

## Overview

Karere relies on stable internet connectivity to communicate with WhatsApp Web servers. This guide helps diagnose and resolve various connectivity issues that may prevent proper operation.

## Quick Network Diagnostics

### Basic Connectivity Check

Before diving into complex troubleshooting, perform these basic checks:

```bash
# 1. Test general internet connectivity
ping -c 4 8.8.8.8

# 2. Test DNS resolution
nslookup web.whatsapp.com

# 3. Test WhatsApp Web connectivity
curl -I https://web.whatsapp.com

# 4. Check your public IP
curl ifconfig.me
```

### WhatsApp Service Status

Verify that WhatsApp services are operational:

1. **Check WhatsApp Status:**
   - Visit [WhatsApp Status](https://downdetector.com/status/whatsapp/)
   - Test WhatsApp Web in a regular browser
   - Check social media for widespread outages

2. **Test Alternative Access:**
   ```bash
   # Try different WhatsApp endpoints
   curl -I https://web.whatsapp.com
   curl -I https://www.whatsapp.com
   ```

## Common Connectivity Issues

### Cannot Connect to WhatsApp Web

#### Issue: "Could not connect to WhatsApp Web" error

**Diagnostic Steps:**
```bash
# Test connectivity to WhatsApp servers
ping web.whatsapp.com
traceroute web.whatsapp.com
mtr --report web.whatsapp.com
```

**Solutions:**

1. **Network Configuration:**
   ```bash
   # Reset network connection
   sudo systemctl restart NetworkManager
   
   # Or for systems using systemd-networkd
   sudo systemctl restart systemd-networkd
   ```

2. **DNS Resolution:**
   ```bash
   # Clear DNS cache
   sudo systemctl flush-dns
   
   # Or manually flush DNS cache
   sudo resolvectl flush-caches
   
   # Test with different DNS servers
   echo 'nameserver 1.1.1.1' | sudo tee /etc/resolv.conf
   echo 'nameserver 8.8.8.8' | sudo tee -a /etc/resolv.conf
   ```

3. **Firewall Configuration:**
   ```bash
   # Check if firewall is blocking connections
   sudo ufw status
   
   # Temporarily disable firewall for testing
   sudo ufw disable
   # Test Karere, then re-enable: sudo ufw enable
   ```

#### Issue: SSL/TLS Certificate Errors

**Symptoms:**
- "Certificate verification failed" errors
- "SSL handshake failed" messages

**Solutions:**

1. **Update CA Certificates:**
   ```bash
   # Ubuntu/Debian
   sudo apt update && sudo apt install ca-certificates
   sudo update-ca-certificates
   
   # Fedora
   sudo dnf update ca-certificates
   
   # Arch Linux
   sudo pacman -S ca-certificates
   ```

2. **Check System Time:**
   ```bash
   # Verify system time is correct
   timedatectl status
   
   # Synchronize time if needed
   sudo timedatectl set-ntp true
   sudo systemctl restart systemd-timesyncd
   ```

3. **Test SSL Connection:**
   ```bash
   # Test SSL connection to WhatsApp
   openssl s_client -connect web.whatsapp.com:443 -servername web.whatsapp.com
   ```

### Intermittent Connection Issues

#### Issue: Frequent disconnections or timeouts

**Diagnostic Commands:**
```bash
# Monitor network stability
ping -i 1 web.whatsapp.com | while read line; do
    echo "$(date): $line"
done

# Check for packet loss
ping -c 100 web.whatsapp.com | tail -3

# Monitor network interface
netstat -i
watch -n 1 'cat /proc/net/dev'
```

**Solutions:**

1. **Network Interface Issues:**
   ```bash
   # Check network interface status
   ip link show
   
   # Reset network interface
   sudo ip link set dev wlan0 down
   sudo ip link set dev wlan0 up
   
   # Or restart NetworkManager
   sudo systemctl restart NetworkManager
   ```

2. **Wi-Fi Specific Issues:**
   ```bash
   # Check Wi-Fi signal strength
   iwconfig wlan0
   
   # Scan for networks and interference
   sudo iwlist wlan0 scan | grep -E 'ESSID|Channel|Signal'
   
   # Check for power management issues
   iwconfig wlan0 power off
   ```

3. **Connection Quality:**
   ```bash
   # Analyze connection quality
   mtr --report --report-cycles 100 web.whatsapp.com
   
   # Check for high latency or jitter
   fping -c 20 -q web.whatsapp.com
   ```

### Proxy and Corporate Network Issues

#### Issue: Cannot connect through corporate proxy

**Diagnostic Steps:**
```bash
# Check proxy environment variables
echo $http_proxy
echo $https_proxy
echo $no_proxy

# Test proxy connectivity
curl -I --proxy $http_proxy https://web.whatsapp.com
```

**Solutions:**

1. **Configure Proxy for Flatpak:**
   ```bash
   # Set proxy for Flatpak applications
   flatpak override --user --env=http_proxy=http://proxy.company.com:8080 io.github.tobagin.karere
   flatpak override --user --env=https_proxy=http://proxy.company.com:8080 io.github.tobagin.karere
   ```

2. **System-wide Proxy Configuration:**
   ```bash
   # Edit system proxy settings
   sudo nano /etc/environment
   
   # Add these lines:
   http_proxy="http://proxy.company.com:8080"
   https_proxy="http://proxy.company.com:8080"
   no_proxy="localhost,127.0.0.1,local.home"
   ```

3. **Proxy Authentication:**
   ```bash
   # For authenticated proxies
   export http_proxy="http://username:password@proxy.company.com:8080"
   export https_proxy="http://username:password@proxy.company.com:8080"
   ```

#### Issue: Blocked ports or restricted access

**Diagnostic Steps:**
```bash
# Check if standard HTTPS port is accessible
telnet web.whatsapp.com 443

# Test alternative ports (if proxy supports)
nc -zv web.whatsapp.com 80
nc -zv web.whatsapp.com 8080
```

**Solutions:**

1. **Contact IT Administration:**
   - Request whitelist access to `*.whatsapp.com`
   - Ensure ports 80 and 443 are accessible
   - Request WebSocket support for real-time messaging

2. **Alternative Access Methods:**
   ```bash
   # Use VPN if company policy allows
   sudo openvpn --config your-vpn-config.ovpn
   
   # Test with mobile hotspot
   # Switch to phone's mobile data temporarily for testing
   ```

## Advanced Network Diagnostics

### Packet Analysis

#### Capture Network Traffic
```bash
# Capture packets for analysis (requires root)
sudo tcpdump -i any -w karere-traffic.pcap host web.whatsapp.com

# Analyze with Wireshark
wireshark karere-traffic.pcap
```

#### Monitor Active Connections
```bash
# Show active connections for Karere
sudo netstat -tulpn | grep karere
sudo ss -tulpn | grep karere

# Monitor connection states
watch -n 1 'ss -tuln | grep ":443"'
```

### WebSocket Connection Issues

#### Issue: WebSocket connections failing

WhatsApp Web uses WebSocket for real-time communication.

**Diagnostic Steps:**
```bash
# Test WebSocket connectivity
wscat -c wss://web.whatsapp.com/ws

# Check WebSocket support through proxy
curl --include \
     --no-buffer \
     --header "Connection: Upgrade" \
     --header "Upgrade: websocket" \
     --header "Sec-WebSocket-Key: SGVsbG8sIHdvcmxkIQ==" \
     --header "Sec-WebSocket-Version: 13" \
     https://web.whatsapp.com/ws
```

**Solutions:**

1. **Proxy WebSocket Support:**
   ```bash
   # Configure proxy for WebSocket tunneling
   # Add to /etc/environment or proxy config:
   # CONNECT_METHOD=tunnel
   # WEBSOCKET_SUPPORT=true
   ```

2. **Firewall Rules for WebSocket:**
   ```bash
   # Allow WebSocket connections
   sudo ufw allow out 443/tcp comment 'WhatsApp HTTPS'
   sudo ufw allow out 80/tcp comment 'WhatsApp HTTP'
   ```

### IPv6 Connectivity Issues

#### Issue: IPv6 causing connection problems

**Diagnostic Steps:**
```bash
# Check IPv6 connectivity
ping6 -c 4 ipv6.google.com

# Test WhatsApp with IPv6
ping6 web.whatsapp.com

# Check IPv6 configuration
ip -6 addr show
```

**Solutions:**

1. **Disable IPv6 Temporarily:**
   ```bash
   # Temporarily disable IPv6
   echo 1 | sudo tee /proc/sys/net/ipv6/conf/all/disable_ipv6
   
   # Permanent disable (add to /etc/sysctl.conf)
   echo 'net.ipv6.conf.all.disable_ipv6 = 1' | sudo tee -a /etc/sysctl.conf
   sudo sysctl -p
   ```

2. **Fix IPv6 Configuration:**
   ```bash
   # Reset IPv6 configuration
   sudo systemctl restart systemd-networkd
   
   # Check for IPv6 address conflicts
   ip -6 route show
   ```

## Mobile Network Specific Issues

### Tethering and Mobile Hotspot

#### Issue: Connection problems when using mobile data

**Diagnostic Steps:**
```bash
# Check mobile connection quality
ping -c 10 8.8.8.8
speedtest-cli

# Monitor data usage
vnstat -i ppp0  # or your mobile interface
```

**Solutions:**

1. **Optimize for Mobile Networks:**
   ```bash
   # Reduce keep-alive frequency for mobile networks
   # This is typically configured in WhatsApp Web itself
   ```

2. **Data Saving Mode:**
   - Enable data saving in WhatsApp Web settings
   - Disable auto-download of media
   - Use lower quality for calls

#### Issue: NAT or carrier-grade NAT problems

**Diagnostic Steps:**
```bash
# Check your public IP vs local IP
curl ifconfig.me
ip addr show

# Test NAT traversal
sudo traceroute -n web.whatsapp.com
```

**Solutions:**

1. **VPN to Bypass CGNAT:**
   ```bash
   # Use VPN service that provides dedicated IP
   sudo openvpn --config vpn-config.ovpn
   ```

2. **Contact Mobile Carrier:**
   - Request dedicated IP address
   - Ask about port forwarding limitations

## ISP and Geographic Restrictions

### Regional Blocking

#### Issue: WhatsApp Web blocked in your region

**Diagnostic Steps:**
```bash
# Test from different locations
curl -I --max-time 10 https://web.whatsapp.com

# Check with VPN from different countries
# Use VPN service and test connectivity
```

**Solutions:**

1. **VPN Configuration:**
   ```bash
   # Install and configure VPN
   sudo apt install openvpn
   
   # Configure VPN with servers in supported regions
   sudo openvpn --config server-us.ovpn
   ```

2. **DNS over HTTPS:**
   ```bash
   # Use DNS over HTTPS to bypass DNS blocking
   sudo systemd-resolve --set-dns=1.1.1.1 --interface=wlan0
   sudo systemd-resolve --set-dns-over-tls=yes --interface=wlan0
   ```

### Traffic Shaping and Throttling

#### Issue: ISP throttling WhatsApp traffic

**Diagnostic Steps:**
```bash
# Compare speeds to different services
speedtest-cli --server 12345  # Your ISP's server
curl -o /dev/null -s -w "%{speed_download}\n" https://web.whatsapp.com

# Test during different times of day
for hour in {0..23}; do
    echo "Hour $hour:"
    curl -o /dev/null -s -w "%{speed_download}\n" https://web.whatsapp.com
    sleep 3600
done
```

**Solutions:**

1. **Contact ISP:**
   - Report throttling issues
   - Request technical support
   - Consider changing ISP plans

2. **Use Alternative Routing:**
   ```bash
   # Route through VPN
   sudo ip route add web.whatsapp.com via vpn-gateway
   ```

## Flatpak Network Permissions

### Sandbox Network Restrictions

#### Issue: Flatpak sandbox blocking network access

**Diagnostic Steps:**
```bash
# Check Flatpak permissions
flatpak info --show-permissions io.github.tobagin.karere

# Test network access from sandbox
flatpak run --command=sh io.github.tobagin.karere
# Inside sandbox:
ping -c 1 web.whatsapp.com
```

**Solutions:**

1. **Grant Network Permissions:**
   ```bash
   # Ensure network permission is granted
   flatpak override --user --share=network io.github.tobagin.karere
   
   # Or system-wide
   sudo flatpak override --share=network io.github.tobagin.karere
   ```

2. **Check Flatpak Network Policy:**
   ```bash
   # Verify no network restrictions
   flatpak permission-show network
   flatpak permission-set network io.github.tobagin.karere yes
   ```

### Portal and Proxy Integration

#### Issue: Flatpak portal network configuration

**Diagnostic Steps:**
```bash
# Check portal status
systemctl --user status xdg-desktop-portal

# Test portal network access
dbus-send --session --dest=org.freedesktop.portal.Desktop \
  --type=method_call --print-reply \
  /org/freedesktop/portal/desktop \
  org.freedesktop.portal.NetworkMonitor.GetConnectivity
```

**Solutions:**

1. **Reset Portal Configuration:**
   ```bash
   # Restart portal services
   systemctl --user restart xdg-desktop-portal
   systemctl --user restart xdg-desktop-portal-gtk
   ```

2. **Manual Portal Configuration:**
   ```bash
   # Configure network portal
   mkdir -p ~/.config/xdg-desktop-portal
   cat > ~/.config/xdg-desktop-portal/portals.conf << EOF
   [preferred]
   default=gtk
   org.freedesktop.impl.portal.NetworkMonitor=gtk
   EOF
   ```

## Network Performance Optimization

### Bandwidth Optimization

#### Optimize for Low Bandwidth
```bash
# Limit bandwidth for Karere (using tc - traffic control)
sudo tc qdisc add dev wlan0 root handle 1: htb default 30
sudo tc class add dev wlan0 parent 1: classid 1:1 htb rate 1mbit ceil 2mbit

# Apply to specific process
sudo tc filter add dev wlan0 protocol ip parent 1:0 prio 1 u32 \
  match ip sport 443 0xffff flowid 1:1
```

#### Quality of Service (QoS)
```bash
# Prioritize WhatsApp traffic
sudo tc qdisc add dev wlan0 root handle 1: prio
sudo tc filter add dev wlan0 parent 1: protocol ip prio 1 u32 \
  match ip dst web.whatsapp.com flowid 1:1
```

### Connection Optimization

#### TCP Optimization
```bash
# Optimize TCP settings for better performance
echo 'net.core.rmem_max = 16777216' | sudo tee -a /etc/sysctl.conf
echo 'net.core.wmem_max = 16777216' | sudo tee -a /etc/sysctl.conf
echo 'net.ipv4.tcp_rmem = 4096 16384 16777216' | sudo tee -a /etc/sysctl.conf
echo 'net.ipv4.tcp_wmem = 4096 16384 16777216' | sudo tee -a /etc/sysctl.conf

# Apply changes
sudo sysctl -p
```

## Monitoring and Logging

### Network Activity Logging

#### Enable Network Logging
```bash
# Monitor network activity for Karere
sudo netstat -c | grep karere

# Log network connections
while true; do
    echo "$(date): $(ss -tuln | grep karere | wc -l) active connections"
    sleep 60
done > karere-connections.log
```

#### Analyze Network Logs
```bash
# Parse connection logs
awk '/ESTABLISHED/ {print $1, $5}' karere-connections.log | sort | uniq -c

# Monitor bandwidth usage
vnstat -i wlan0 --json | jq '.interfaces[0].traffic.total'
```

### Automated Network Monitoring

#### Network Health Check Script
```bash
#!/bin/bash
# karere-network-check.sh

LOG_FILE="/tmp/karere-network.log"
WHATSAPP_HOST="web.whatsapp.com"

log_message() {
    echo "$(date): $1" >> "$LOG_FILE"
}

# Check basic connectivity
if ping -c 1 "$WHATSAPP_HOST" > /dev/null 2>&1; then
    log_message "Connectivity: OK"
else
    log_message "Connectivity: FAILED"
fi

# Check DNS resolution
if nslookup "$WHATSAPP_HOST" > /dev/null 2>&1; then
    log_message "DNS: OK"
else
    log_message "DNS: FAILED"
fi

# Check HTTPS connectivity
if curl -I "https://$WHATSAPP_HOST" > /dev/null 2>&1; then
    log_message "HTTPS: OK"
else
    log_message "HTTPS: FAILED"
fi

# Check Karere process
if pgrep karere > /dev/null; then
    log_message "Karere: Running"
else
    log_message "Karere: Not running"
fi
```

## Getting Help with Network Issues

### Information to Collect

When reporting network connectivity issues, collect:

1. **Network Configuration:**
   ```bash
   ip addr show
   ip route show
   cat /etc/resolv.conf
   ```

2. **Connectivity Tests:**
   ```bash
   ping -c 4 web.whatsapp.com
   traceroute web.whatsapp.com
   curl -I https://web.whatsapp.com
   ```

3. **System Information:**
   ```bash
   uname -a
   lsb_release -a
   systemctl status NetworkManager
   ```

4. **Karere Logs:**
   ```bash
   journalctl --user -u app.flatpak.io.github.tobagin.karere
   ```

### Expert Network Analysis

For complex network issues:

1. **Use Wireshark for Packet Analysis**
2. **Contact Network Administrator (for corporate networks)**
3. **Test with Different Network Connections**
4. **Document Intermittent Issues with Timestamps**

---

*For additional networking issues not covered here, see [Common Issues](common-issues.md) or consult the [FAQ](../FAQ.md).*