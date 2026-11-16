import { describe, it, expect } from 'vitest'
import { formatBytes, formatMbps, buildMarkdownReport, type DiagnosticInput } from './report'

const base: DiagnosticInput = {
  generatedAt: new Date('2026-11-17T10:00:00Z'),
  hostname: 'pc',
  ipv4: '192.168.1.10',
  ipv6: null,
  interfaces: [],
  connectivity: null,
  publicIp: null,
  ping: null,
  dnsServers: null,
  dnsLookup: null,
  speed: null,
}

describe('formatBytes / formatMbps', () => {
  it('formats', () => {
    expect(formatBytes(500)).toBe('500 B')
    expect(formatBytes(8 * 1024 * 1024)).toBe('8.00 MB')
    expect(formatMbps(87.25)).toBe('87.3 Mbps')
    expect(formatMbps(2400)).toBe('2.40 Gbps')
  })
})

describe('buildMarkdownReport', () => {
  it('marks unrun checks as _not run_ — never fabricates values', () => {
    const md = buildMarkdownReport(base)
    expect(md).toContain('Connectivity check: _not run_')
    expect(md).toContain('Ping: _not run_')
    expect(md).toContain('Resolvers: _not read_')
    expect(md).toContain('Speed test: _not run_')
    expect(md).toContain('Local IPv4: 192.168.1.10')
    expect(md).toContain('Hostname: pc')
  })

  it('includes real results when provided', () => {
    const md = buildMarkdownReport({
      ...base,
      connectivity: { online: true, latency_ms: 11, target: '1.1.1.1:443' },
      publicIp: '203.0.113.7',
      ping: { host: 'example.com', count: 4, loss_percent: 0, min_ms: 20, avg_ms: 30, max_ms: 40, per_reply: [20, 30, 40], ranAt: new Date('2026-11-17T10:05:00Z') },
      dnsServers: ['1.1.1.1'],
      dnsLookup: { host: 'example.com', ips: ['93.184.216.34'] },
      speed: { mbps: 94.1, bytes: 8000000, seconds: 0.68, endpoint: 'speed.cloudflare.com/__down?bytes=8000000' },
    })
    expect(md).toContain('online, TCP connect in 11.0 ms')
    expect(md).toContain('93.184.216.34')
    expect(md).toContain('94.1 Mbps')
    expect(md).toContain('203.0.113.7')
    expect(md).toContain('Packet loss: 0%')
  })
})
