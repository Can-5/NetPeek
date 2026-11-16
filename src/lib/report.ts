// NetPeek — display formatting + markdown diagnostic report builder.
// Pure functions, unit-tested. Nothing here fabricates network data:
// sections for checks the user never ran are marked "_not run_".

export interface DiagnosticInput {
  generatedAt: Date
  hostname: string
  ipv4: string | null
  ipv6: string | null
  interfaces: { name: string; addr: string; kind: string }[]
  connectivity: { online: boolean; latency_ms: number | null; target: string; error?: string } | null
  publicIp: string | null
  ping: {
    host: string
    count: number
    loss_percent: number
    min_ms: number | null
    avg_ms: number | null
    max_ms: number | null
    per_reply: number[]
    ranAt: Date
  } | null
  dnsServers: string[] | null
  dnsLookup: { host: string; ips: string[] } | null
  speed: { mbps: number; bytes: number; seconds: number; endpoint: string } | null
}

export function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes < 0) return 'unknown'
  if (bytes < 1024) return `${bytes} B`
  const units = ['KB', 'MB', 'GB']
  let value = bytes / 1024
  let unit = units[0]
  for (let i = 1; i < units.length && value >= 1024; i++) {
    value /= 1024
    unit = units[i]
  }
  return `${value.toFixed(value >= 100 ? 0 : value >= 10 ? 1 : 2)} ${unit}`
}

export function formatMbps(mbps: number): string {
  if (!Number.isFinite(mbps)) return '—'
  return mbps >= 1000 ? `${(mbps / 1000).toFixed(2)} Gbps` : `${mbps.toFixed(1)} Mbps`
}

function ms(v: number | null | undefined): string {
  return v == null ? '—' : `${v.toFixed(1)} ms`
}

export function buildMarkdownReport(d: DiagnosticInput): string {
  const lines: string[] = []
  lines.push('# NetPeek Diagnostic Report', '')
  lines.push(`Generated: ${d.generatedAt.toISOString()}`, '')

  lines.push('## System', '')
  lines.push(`- Hostname: ${d.hostname}`)
  lines.push(`- Local IPv4: ${d.ipv4 ?? 'not detected'}`)
  lines.push(`- Local IPv6: ${d.ipv6 ?? 'not detected'}`)
  if (d.interfaces.length) {
    lines.push('- Interfaces:')
    for (const i of d.interfaces) lines.push(`  - ${i.name}: ${i.addr} (${i.kind})`)
  } else {
    lines.push('- Interfaces: none reported (loopbacks excluded)')
  }
  lines.push('')

  lines.push('## Connectivity', '')
  if (d.connectivity) {
    const c = d.connectivity
    lines.push(`- ${c.target}: ${c.online ? `online, TCP connect in ${ms(c.latency_ms)}` : `offline (${c.error ?? 'unreachable'})`}`)
  } else {
    lines.push('- Connectivity check: _not run_')
  }
  lines.push(`- Public IP: ${d.publicIp ?? '_not fetched_'}`)
  lines.push('')

  lines.push('## Ping', '')
  if (d.ping) {
    const p = d.ping
    lines.push(`- Host: ${p.host} (${p.count} packets, ${p.ranAt.toISOString()})`)
    lines.push(`- Packet loss: ${p.loss_percent.toFixed(0)}%`)
    lines.push(`- RTT min/avg/max: ${ms(p.min_ms)} / ${ms(p.avg_ms)} / ${ms(p.max_ms)}`)
  } else {
    lines.push('- Ping: _not run_')
  }
  lines.push('')

  lines.push('## DNS', '')
  if (d.dnsServers) {
    lines.push(`- Resolvers: ${d.dnsServers.join(', ')}`)
  } else {
    lines.push('- Resolvers: _not read_')
  }
  if (d.dnsLookup) {
    lines.push(`- Lookup ${d.dnsLookup.host}: ${d.dnsLookup.ips.join(', ')}`)
  } else {
    lines.push('- Lookup: _not run_')
  }
  lines.push('')

  lines.push('## Speed test', '')
  if (d.speed) {
    const s = d.speed
    lines.push(`- Download: ${formatMbps(s.mbps)} (${formatBytes(s.bytes)} in ${s.seconds.toFixed(2)} s)`)
    lines.push(`- Endpoint: ${s.endpoint}`)
  } else {
    lines.push('- Speed test: _not run_')
  }
  lines.push('')
  lines.push('---', 'Generated locally by NetPeek. No data was stored remotely.', '')
  return lines.join('\n')
}
