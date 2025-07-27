# Performance Optimization Guide

> Maximize Karere's performance and efficiency on your system

## Overview

Karere is designed to be lightweight and efficient, but performance can be affected by various factors including system specifications, configuration, and usage patterns. This guide provides comprehensive strategies to optimize Karere's performance.

## Performance Monitoring

### Built-in Performance Tools

#### Resource Usage Display
Monitor Karere's resource consumption:

1. **Memory Usage:**
   ```bash
   # Check Karere's memory usage
   ps aux | grep karere | awk '{print $4, $6, $11}'
   
   # Detailed memory breakdown
   pmap $(pgrep karere)
   ```

2. **CPU Usage:**
   ```bash
   # Real-time CPU monitoring
   top -p $(pgrep karere)
   
   # CPU usage over time
   pidstat -p $(pgrep karere) 1 10
   ```

3. **Network Activity:**
   ```bash
   # Network connections
   netstat -p | grep karere
   
   # Bandwidth usage (requires nethogs)
   sudo nethogs
   ```

#### Performance Metrics
Access detailed performance information:
- **Developer Tools:** Press `Ctrl+Shift+I` (if enabled in preferences)
- **WebKit Performance:** Monitor web rendering performance
- **Memory Profiler:** Analyze memory allocation patterns

### System Monitoring Tools

#### Command Line Tools
```bash
# System-wide performance overview
htop

# Detailed system statistics
iostat 1 5

# Memory usage analysis
free -h && sync && echo 3 > /proc/sys/vm/drop_caches && free -h

# Disk I/O monitoring
iotop
```

#### GUI Monitoring Tools
- **GNOME System Monitor:** Visual resource monitoring
- **KSysGuard:** KDE system monitoring (for Plasma users)
- **Stacer:** System optimizer and monitor

## Memory Optimization

### Understanding Memory Usage

#### Memory Components
Karere's memory usage consists of:
- **Application Memory:** Core Karere application (~20-50MB)
- **WebKit Engine:** Web rendering engine (~100-300MB)
- **Web Content:** WhatsApp Web page and data (~50-200MB)
- **Cache Data:** Temporary files and cached content (~10-100MB)

#### Normal Memory Usage
Typical memory consumption:
- **Minimal Usage:** 200-400MB
- **Normal Usage:** 400-800MB
- **Heavy Usage:** 800MB-1.5GB

### Memory Optimization Strategies

#### 1. Cache Management
Regular cache cleanup prevents memory bloat:

```bash
# Clear all cache data
rm -rf ~/.var/app/io.github.tobagin.karere/cache/*

# Clear web cache specifically
rm -rf ~/.var/app/io.github.tobagin.karere/data/webkit/WebKitCache/*

# Clear local storage
rm -rf ~/.var/app/io.github.tobagin.karere/data/webkit/LocalStorage/*
```

**Automated Cache Cleanup:**
Create a script for regular maintenance:
```bash
#!/bin/bash
# karere-cleanup.sh
echo "Cleaning Karere cache..."
rm -rf ~/.var/app/io.github.tobagin.karere/cache/*
rm -rf ~/.var/app/io.github.tobagin.karere/data/webkit/WebKitCache/*
echo "Cache cleanup complete."
```

#### 2. Memory-Constrained Systems
For systems with limited RAM (4GB or less):

**System Configuration:**
```bash
# Increase swap file size
sudo fallocate -l 2G /swapfile
sudo chmod 600 /swapfile
sudo mkswap /swapfile
sudo swapon /swapfile

# Optimize swappiness
echo 'vm.swappiness=10' | sudo tee -a /etc/sysctl.conf
```

**Karere Settings:**
- Close application when not actively using WhatsApp
- Reduce notification history retention
- Disable debug logging

#### 3. WebKit Memory Management
Optimize WebKit rendering engine:

**Environment Variables:**
```bash
# Limit WebKit memory usage
export WEBKIT_DISABLE_COMPOSITING_MODE=1
export WEBKIT_FORCE_SANDBOX=1

# Run Karere with memory limits
systemd-run --user --scope -p MemoryMax=1G flatpak run io.github.tobagin.karere
```

**Web Content Optimization:**
- Limit open chat conversations
- Clear chat history periodically
- Disable media auto-download

## CPU Optimization

### CPU Usage Analysis

#### Identifying CPU Bottlenecks
```bash
# CPU usage by process
ps -eo pid,ppid,cmd,%mem,%cpu --sort=-%cpu | head -20

# Thread-level analysis
ps -eLf | grep karere

# CPU frequency and scaling
cpufreq-info
```

#### Performance Profiling
```bash
# Profile CPU usage with perf
perf record -g flatpak run io.github.tobagin.karere
perf report

# System call tracing
strace -p $(pgrep karere) -c
```

### CPU Optimization Strategies

#### 1. Reduce Background Processing
Minimize unnecessary CPU usage:

**WebKit Optimization:**
- Disable JavaScript in idle tabs (not always possible with WhatsApp)
- Limit concurrent media processing
- Reduce animation and visual effects

**System Settings:**
```bash
# Set CPU governor to performance
echo performance | sudo tee /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor

# Or use powersave for laptops
echo powersave | sudo tee /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor
```

#### 2. Process Priority Optimization
Adjust process scheduling:

```bash
# Increase Karere priority (use carefully)
sudo renice -10 $(pgrep karere)

# Or decrease priority to be less intrusive
sudo renice 10 $(pgrep karere)

# Set CPU affinity (multi-core systems)
taskset -cp 0,1 $(pgrep karere)
```

#### 3. Graphics and Rendering
Optimize visual rendering:

**Hardware Acceleration:**
```bash
# Check for hardware acceleration support
glxinfo | grep -i "direct rendering"
vainfo  # Video acceleration info
```

**Disable Visual Effects:**
```bash
# GNOME: Disable animations
gsettings set org.gnome.desktop.interface enable-animations false

# Reduce compositor effects
gsettings set org.gnome.mutter experimental-features "[]"
```

## Network Performance

### Network Optimization

#### Connection Monitoring
```bash
# Monitor network connections
ss -tuln | grep karere

# Bandwidth usage tracking
vnstat -i wlan0 -l

# Connection quality testing
mtr web.whatsapp.com
```

#### Network Configuration
Optimize network settings:

```bash
# TCP optimization (system-wide)
echo 'net.core.rmem_max = 16777216' | sudo tee -a /etc/sysctl.conf
echo 'net.core.wmem_max = 16777216' | sudo tee -a /etc/sysctl.conf
echo 'net.ipv4.tcp_rmem = 4096 65536 16777216' | sudo tee -a /etc/sysctl.conf

# Apply changes
sudo sysctl -p
```

#### DNS Optimization
```bash
# Use faster DNS servers
echo 'nameserver 1.1.1.1' | sudo tee /etc/resolv.conf
echo 'nameserver 8.8.8.8' | sudo tee -a /etc/resolv.conf

# Or use systemd-resolved
sudo systemctl enable --now systemd-resolved
```

### Bandwidth Management

#### Reduce Data Usage
For limited bandwidth connections:

1. **Disable Auto-Downloads:**
   - Configure WhatsApp Web to not auto-download media
   - Manually download only necessary files

2. **Optimize Media Quality:**
   - Use lower quality for video calls
   - Compress images before sending

3. **Connection Management:**
   ```bash
   # Monitor data usage
   iftop -i wlan0
   
   # Set bandwidth limits (requires tc - traffic control)
   sudo tc qdisc add dev wlan0 root handle 1: htb default 30
   sudo tc class add dev wlan0 parent 1: classid 1:1 htb rate 1mbit
   ```

## Storage Optimization

### Disk Usage Analysis

#### Monitor Storage Usage
```bash
# Check Karere's storage usage
du -sh ~/.var/app/io.github.tobagin.karere/

# Detailed breakdown
du -h ~/.var/app/io.github.tobagin.karere/ | sort -hr

# Monitor disk I/O
iotop -p $(pgrep karere)
```

#### Disk Space Management
```bash
# Find large files
find ~/.var/app/io.github.tobagin.karere/ -type f -size +10M -exec ls -lh {} \;

# Clean temporary files
find ~/.var/app/io.github.tobagin.karere/ -name "*.tmp" -delete
find ~/.var/app/io.github.tobagin.karere/ -name "*.log" -mtime +30 -delete
```

### Storage Optimization Strategies

#### 1. Log Management
Configure log retention:

```bash
# Automatic log rotation configuration
cat > ~/.var/app/io.github.tobagin.karere/config/logrotate.conf << EOF
~/.var/app/io.github.tobagin.karere/data/logs/*.log {
    daily
    rotate 7
    compress
    delaycompress
    missingok
    notifempty
}
EOF
```

#### 2. Cache Optimization
Smart cache management:

```bash
# Cache size monitoring
du -sh ~/.var/app/io.github.tobagin.karere/cache/

# Selective cache cleanup
find ~/.var/app/io.github.tobagin.karere/cache/ -atime +7 -delete
```

#### 3. Database Optimization
If Karere uses local databases:

```bash
# SQLite optimization (if applicable)
sqlite3 ~/.var/app/io.github.tobagin.karere/data/karere.db "VACUUM;"
sqlite3 ~/.var/app/io.github.tobagin.karere/data/karere.db "ANALYZE;"
```

## System-Level Optimizations

### Operating System Tuning

#### Kernel Parameters
```bash
# I/O scheduler optimization
echo noop | sudo tee /sys/block/sda/queue/scheduler

# Memory management
echo 1 | sudo tee /proc/sys/vm/drop_caches
echo 'vm.dirty_ratio = 5' | sudo tee -a /etc/sysctl.conf
echo 'vm.dirty_background_ratio = 2' | sudo tee -a /etc/sysctl.conf
```

#### File System Optimization
```bash
# Check file system
sudo fsck -f /dev/sda1

# Mount options for performance
# Add 'noatime,nodiratime' to /etc/fstab for the root partition
```

### Desktop Environment Optimization

#### GNOME Optimization
```bash
# Disable unnecessary services
systemctl --user disable evolution-data-server
systemctl --user disable tracker-miner-fs

# Optimize GNOME Shell
gsettings set org.gnome.shell.overrides workspaces-only-on-primary false
gsettings set org.gnome.desktop.interface gtk-im-module 'gtk-im-context-simple'
```

#### KDE Plasma Optimization
```bash
# Disable compositor temporarily
qdbus org.kde.KWin /Compositor suspend

# Reduce desktop effects
kwriteconfig5 --file ~/.config/kwinrc --group Compositing --key Enabled false
```

## Troubleshooting Performance Issues

### Common Performance Problems

#### 1. High Memory Usage
**Symptoms:** System becomes slow, applications start swapping
**Solutions:**
- Regular cache cleanup
- Restart Karere daily
- Check for memory leaks in Developer Tools
- Monitor background processes

#### 2. CPU Spikes
**Symptoms:** Fan noise, system heat, battery drain
**Solutions:**
- Profile with `perf` or `htop`
- Check for JavaScript loops in web content
- Disable hardware acceleration if problematic
- Update to latest version

#### 3. Slow Network Performance
**Symptoms:** Slow message delivery, poor call quality
**Solutions:**
- Test network with other applications
- Check firewall/proxy settings
- Monitor network quality with `mtr`
- Consider VPN if connectivity issues persist

### Performance Regression Investigation

#### Benchmarking
Create performance benchmarks:

```bash
#!/bin/bash
# performance-test.sh
echo "=== Karere Performance Test ==="
echo "Date: $(date)"
echo "System: $(uname -a)"

# Startup time
echo "Testing startup time..."
time timeout 30 flatpak run io.github.tobagin.karere --startup-test

# Memory usage after 5 minutes
echo "Memory usage baseline..."
sleep 300
ps aux | grep karere | awk '{print "Memory: " $6 "KB, CPU: " $3 "%"}'

# Network latency
echo "Network performance..."
ping -c 10 web.whatsapp.com | tail -1
```

#### Comparing Versions
When updating Karere:
1. Document current performance metrics
2. Test new version in parallel (development vs stable)
3. Compare resource usage and responsiveness
4. Report performance regressions to developers

## Advanced Performance Tuning

### Custom Launch Scripts

#### Optimized Launch Configuration
```bash
#!/bin/bash
# karere-optimized.sh

# Set environment variables
export WEBKIT_DISABLE_COMPOSITING_MODE=1
export WEBKIT_FORCE_SANDBOX=1
export GDK_BACKEND=wayland  # Use Wayland if available

# Set process limits
ulimit -m 1048576  # Limit memory to 1GB
ulimit -u 100      # Limit processes

# Launch with nice priority
nice -n 5 flatpak run io.github.tobagin.karere "$@"
```

#### Resource-Constrained Systems
```bash
#!/bin/bash
# karere-lowres.sh

# Minimal resource configuration
export WEBKIT_DISABLE_WEBGL=1
export WEBKIT_DISABLE_ACCELERATED_COMPOSITING=1
export GDK_SCALE=1

# Force software rendering
export LIBGL_ALWAYS_SOFTWARE=1

# Launch with memory constraints
systemd-run --user --scope \
  -p MemoryMax=512M \
  -p CPUQuota=50% \
  flatpak run io.github.tobagin.karere
```

### Custom Configurations

#### Performance-Focused Settings
Create custom configuration files:

```ini
# ~/.var/app/io.github.tobagin.karere/config/performance.conf
[webkit]
enable-hardware-acceleration=false
enable-smooth-scrolling=false
enable-webgl=false

[logging]
log-level=error
max-log-size=10MB

[cache]
max-cache-size=100MB
cache-cleanup-interval=3600
```

### Monitoring and Alerting

#### Automated Performance Monitoring
```bash
#!/bin/bash
# karere-monitor.sh

MEMORY_THRESHOLD=800000  # 800MB in KB
CPU_THRESHOLD=50         # 50% CPU

while true; do
    MEMORY_USAGE=$(ps aux | grep '[k]arere' | awk '{sum += $6} END {print sum}')
    CPU_USAGE=$(ps aux | grep '[k]arere' | awk '{sum += $3} END {print sum}')
    
    if [ "$MEMORY_USAGE" -gt "$MEMORY_THRESHOLD" ]; then
        notify-send "Karere Performance Warning" "High memory usage: ${MEMORY_USAGE}KB"
    fi
    
    if [ "${CPU_USAGE%.*}" -gt "$CPU_THRESHOLD" ]; then
        notify-send "Karere Performance Warning" "High CPU usage: ${CPU_USAGE}%"
    fi
    
    sleep 60
done
```

## Performance Best Practices

### Daily Usage Tips

1. **Regular Maintenance:**
   - Restart Karere at least once daily
   - Clear cache weekly
   - Monitor resource usage

2. **Efficient Usage Patterns:**
   - Close application when not needed
   - Limit concurrent media downloads
   - Use minimal window size when possible

3. **System Optimization:**
   - Keep system updated
   - Monitor background processes
   - Maintain adequate free disk space (>10%)

### Long-term Performance

1. **Preventive Measures:**
   - Regular system maintenance
   - Monitor performance trends
   - Update software regularly

2. **Capacity Planning:**
   - Monitor resource usage growth
   - Plan for increased data usage
   - Consider hardware upgrades

3. **Performance Monitoring:**
   - Set up automated monitoring
   - Create performance baselines
   - Document performance changes

---

*For additional performance issues, see [Common Issues](common-issues.md) or report performance problems on [GitHub](https://github.com/tobagin/karere-vala/issues).*