<script lang="ts">
  import { onMount } from 'svelte'
  import { invoke } from '@tauri-apps/api/core'
  import { cycleTheme } from '$lib/theme'
  import { buildMarkdownReport, formatBytes, formatMbps, type DiagnosticInput } from '$lib/report'

  interface LocalInfo {
    hostname: string
    ipv4: string | null
    ipv6: string | null
    interfaces: { name: string; addr: string; kind: string }[]
  }
  interface PingResult {
    transmitted: number
    received: number
    loss_percent: number
    min_ms: number | null
    avg_ms: number | null
    max_ms: number | null
    per_reply: number[]
  }
  interface LatencyEntry { ts: number; host: string; avg_ms: number | null; loss_percent: number }

  const PERSIST_KEY = 'netpeek:last'

  let local = $state<LocalInfo | null>(null)
  let loadingLocal = $state(true)

  let pingHost = $state('1.1.1.1')
  let pingCount = $state('4')
  let pingResult = $state<PingResult | null>(null)
  let pingError = $state('')
  let pingRunning = $state(false)
  let pingRanAt = $state<Date | null>(null)

  let connectivity = $state<{ online: boolean; latency_ms: number | null; target: string; error?: string } | null>(null)
  let connectivityRunning = $state(false)
  let connectivityError = $state('')

  let publicIp = $state<string | null>(null)
  let publicIpRunning = $state(false)
  let publicIpError = $state('')

  let speed = $state<{ mbps: number; bytes: number; seconds: number; endpoint: string } | null>(null)
  let speedRunning = $state(false)
  let speedError = $state('')

  let dnsServers = $state<string[] | null>(null)
  let dnsError = $state('')
  let lookupHost = $state('example.com')
  let lookupResult = $state<string[] | null>(null)
  let lookupError = $state('')
  let lookupRunning = $state(false)

  let history = $state<LatencyEntry[]>([])

  let exportMsg = $state('')
  let toast = $state('')
  let toastTimer: ReturnType<typeof setTimeout> | undefined

  function showToast(msg: string) {
    toast = msg
    clearTimeout(toastTimer)
    toastTimer = setTimeout(() => (toast = ''), 3600)
  }

  function persistAll() {
    try {
      localStorage.setItem(PERSIST_KEY, JSON.stringify({ pingHost, pingResult, pingRanAt, connectivity, publicIp, speed, dnsServers, lookupHost, lookupResult }))
    } catch { /* ignore */ }
  }

  async function refreshHistory() {
    try {
      history = await invoke<LatencyEntry[]>('latency_history', { limit: 20 })
    } catch {
      history = []
    }
  }

  async function loadLocal() {
    loadingLocal = true
    try {
      local = await invoke<LocalInfo>('local_info')
    } catch (e) {
      showToast(`Could not read local network info: ${e}`)
    } finally {
      loadingLocal = false
    }
  }

  async function runPing() {
    pingError = ''
    pingRunning = true
    try {
      pingResult = await invoke<PingResult>('ping_host', { host: pingHost, count: Number(pingCount) })
      pingRanAt = new Date()
      await refreshHistory()
      persistAll()
    } catch (e) {
      pingResult = null
      pingError = String(e)
    } finally {
      pingRunning = false
    }
  }

  async function runConnectivity() {
    connectivityError = ''
    connectivityRunning = true
    try {
      connectivity = await invoke('connection_check')
      persistAll()
    } catch (e) {
      connectivityError = String(e)
    } finally {
      connectivityRunning = false
    }
  }

  async function runPublicIp() {
    publicIpError = ''
    publicIpRunning = true
    try {
      publicIp = await invoke<string>('public_ip')
      persistAll()
    } catch (e) {
      publicIpError = String(e)
    } finally {
      publicIpRunning = false
    }
  }

  async function runSpeed() {
    speedError = ''
    speedRunning = true
    try {
      speed = await invoke('speed_test')
      persistAll()
    } catch (e) {
      speedError = String(e)
    } finally {
      speedRunning = false
    }
  }

  async function loadDnsServers() {
    dnsError = ''
    try {
      dnsServers = await invoke<string[]>('dns_servers')
      persistAll()
    } catch (e) {
      dnsError = String(e)
    }
  }

  async function runLookup() {
    lookupError = ''
    lookupRunning = true
    try {
      lookupResult = await invoke<string[]>('dns_lookup', { host: lookupHost })
      persistAll()
    } catch (e) {
      lookupResult = null
      lookupError = String(e)
    } finally {
      lookupRunning = false
    }
  }

  async function clearHistory() {
    try {
      await invoke('clear_latency_history')
      history = []
      showToast('Latency history cleared')
    } catch (e) {
      showToast(`Could not clear: ${e}`)
    }
  }

  async function exportReport() {
    if (!local) {
      showToast('Local info not loaded yet')
      return
    }
    const input: DiagnosticInput = {
      generatedAt: new Date(),
      hostname: local.hostname,
      ipv4: local.ipv4,
      ipv6: local.ipv6,
      interfaces: local.interfaces,
      connectivity,
      publicIp,
      ping: pingResult && pingRanAt
        ? { host: pingHost, count: Number(pingCount), loss_percent: pingResult.loss_percent, min_ms: pingResult.min_ms, avg_ms: pingResult.avg_ms, max_ms: pingResult.max_ms, per_reply: pingResult.per_reply, ranAt: pingRanAt }
        : null,
      dnsServers,
      dnsLookup: lookupResult ? { host: lookupHost, ips: lookupResult } : null,
      speed,
    }
    try {
      const path = await invoke<string>('write_export', {
        filename: `netpeek-diagnostic-${new Date().toISOString().slice(0, 10)}.md`,
        content: buildMarkdownReport(input),
      })
      exportMsg = `Saved to ${path}`
      showToast('Diagnostic report saved to Downloads')
    } catch (e) {
      showToast(`Export failed: ${e}`)
    }
  }

  function copyText(text: string) {
    navigator.clipboard?.writeText(text)
    showToast('Copied to clipboard')
  }

  const maxReply = $derived(pingResult ? Math.max(...pingResult.per_reply, 1) : 1)
  const maxHistory = $derived(history.reduce((m, h) => Math.max(m, h.avg_ms ?? 0), 1))

  onMount(async () => {
    try {
      const saved = localStorage.getItem(PERSIST_KEY)
      if (saved) {
        const s = JSON.parse(saved)
        pingHost = s.pingHost ?? pingHost
        pingResult = s.pingResult ?? null
        pingRanAt = s.pingRanAt ? new Date(s.pingRanAt) : null
        connectivity = s.connectivity ?? null
        publicIp = s.publicIp ?? null
        speed = s.speed ?? null
        dnsServers = s.dnsServers ?? null
        lookupHost = s.lookupHost ?? lookupHost
        lookupResult = s.lookupResult ?? null
      }
    } catch { /* corrupt persisted state — ignore */ }
    await loadLocal()
    await loadDnsServers()
    await refreshHistory()
  })
</script>

<div class="app">
  <header class="topbar">
    <div class="brand">
      <span class="logo" aria-hidden="true">
        <svg viewBox="0 0 24 24" width="19" height="19" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"><circle cx="12" cy="12" r="1.5" fill="currentColor" stroke="none"/><path d="M7 7a7 7 0 0 0 0 10M17 7a7 7 0 0 1 0 10M4.5 4.5a10.5 10.5 0 0 0 0 15M19.5 4.5a10.5 10.5 0 0 1 0 15"/></svg>
      </span>
      <div>
        <h1>NetPeek</h1>
        <p>network diagnostics</p>
      </div>
    </div>
    <button class="icon-btn" onclick={cycleTheme} title="Cycle theme" aria-label="Cycle theme">
      <svg viewBox="0 0 24 24" width="17" height="17" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"><circle cx="12" cy="12" r="4.5"/><path d="M12 2.5v2.5M12 19v2.5M2.5 12H5M19 12h2.5M4.6 4.6l1.8 1.8M17.6 17.6l1.8 1.8M4.6 19.4l1.8-1.8M17.6 6.4l1.8-1.8"/></svg>
    </button>
  </header>

  <main class="scroll">
    <!-- This computer -->
    <section class="card">
      <div class="card-head"><h2>This computer</h2>
        {#if loadingLocal}<span class="dim">reading…</span>{/if}
      </div>
      <div class="kv-grid">
        <div class="kv"><span class="k">Hostname</span><span class="v mono">{local?.hostname ?? '—'}</span></div>
        <div class="kv"><span class="k">Local IPv4</span>
          <span class="v mono">{local?.ipv4 ?? 'not detected'}{#if local?.ipv4}<button class="mini-btn" onclick={() => copyText(local!.ipv4!)}>copy</button>{/if}</span>
        </div>
        <div class="kv"><span class="k">Local IPv6</span>
          <span class="v mono">{local?.ipv6 ?? 'not detected'}{#if local?.ipv6}<button class="mini-btn" onclick={() => copyText(local!.ipv6!)}>copy</button>{/if}</span>
        </div>
      </div>
      {#if local?.interfaces.length}
        <div class="iface-table">
          <div class="iface-row iface-head"><span>Interface</span><span>Address</span><span>Type</span></div>
          {#each local.interfaces as iface (iface.name + iface.addr)}
            <div class="iface-row"><span class="mono">{iface.name}</span><span class="mono">{iface.addr}</span><span class="chip">{iface.kind}</span></div>
          {/each}
        </div>
      {:else if !loadingLocal}
        <p class="dim small">No non-loopback interface addresses reported.</p>
      {/if}
    </section>

    <!-- Connectivity + public IP -->
    <div class="two-col">
      <section class="card">
        <div class="card-head"><h2>Connectivity</h2></div>
        <p class="dim small">TCP connect to <code>1.1.1.1:443</code> (Cloudflare). Runs only when you click the button.</p>
        <button class="btn" onclick={runConnectivity} disabled={connectivityRunning}>{connectivityRunning ? 'Checking…' : 'Check connection'}</button>
        {#if connectivity}
          <p class="result-line"><span class="dot" class:ok={connectivity.online} class:bad={!connectivity.online}></span>
            {connectivity.online ? `Online — TCP connect in ${connectivity.latency_ms} ms` : `Offline — ${connectivity.error ?? 'unreachable'}`}
          </p>
        {/if}
        {#if connectivityError}<p class="error-box">{connectivityError}</p>{/if}
      </section>

      <section class="card">
        <div class="card-head"><h2>Public IP</h2></div>
        <p class="dim small">Shows what websites see. Fetched from <code>ifconfig.me</code> only when you click.</p>
        <button class="btn" onclick={runPublicIp} disabled={publicIpRunning}>{publicIpRunning ? 'Looking up…' : 'Show my public IP'}</button>
        {#if publicIp}<p class="result-line mono big">{publicIp}</p>{/if}
        {#if publicIpError}<p class="error-box">{publicIpError}</p>{/if}
      </section>
    </div>

    <!-- Ping -->
    <section class="card">
      <div class="card-head"><h2>Ping</h2><span class="dim small">saved to latency history</span></div>
      <form class="ping-form" onsubmit={(e) => { e.preventDefault(); runPing() }}>
        <input class="input" bind:value={pingHost} placeholder="example.com or 1.1.1.1" aria-label="Host to ping" spellcheck="false" autocomplete="off" />
        <select class="input select" bind:value={pingCount} aria-label="Packet count">
          <option value="4">4 packets</option><option value="8">8 packets</option><option value="16">16 packets</option>
        </select>
        <button class="btn" type="submit" disabled={pingRunning}>{pingRunning ? 'Pinging…' : 'Ping'}</button>
      </form>
      {#if pingError}<p class="error-box">{pingError}</p>{/if}
      {#if pingResult}
        <div class="ping-result">
          <div class="ping-stats">
            <div class="stat"><span class="num">{pingResult.received}/{pingResult.transmitted}</span><span class="lbl">received</span></div>
            <div class="stat"><span class="num">{pingResult.loss_percent.toFixed(0)}%</span><span class="lbl">loss</span></div>
            <div class="stat"><span class="num">{pingResult.min_ms?.toFixed(1) ?? '—'}</span><span class="lbl">min ms</span></div>
            <div class="stat"><span class="num">{pingResult.avg_ms?.toFixed(1) ?? '—'}</span><span class="lbl">avg ms</span></div>
            <div class="stat"><span class="num">{pingResult.max_ms?.toFixed(1) ?? '—'}</span><span class="lbl">max ms</span></div>
          </div>
          {#if pingResult.per_reply.length}
            <div class="reply-bars" title="per-reply RTT">
              {#each pingResult.per_reply as rtt, i (i)}
                <div class="reply-bar" style:height="{(rtt / maxReply) * 100}%" title="seq {i + 1}: {rtt.toFixed(1)} ms"></div>
              {/each}
            </div>
          {/if}
        </div>
      {/if}
    </section>

    <!-- DNS -->
    <div class="two-col">
      <section class="card">
        <div class="card-head"><h2>DNS resolvers</h2>
          <button class="mini-btn" onclick={loadDnsServers}>refresh</button>
        </div>
        {#if dnsServers}<ul class="dns-list mono">{#each dnsServers as s (s)}<li>{s}</li>{/each}</ul>
        {:else if !dnsError}<p class="dim small">No resolvers found.</p>{/if}
        {#if dnsError}<p class="error-box">{dnsError}</p>{/if}
      </section>
      <section class="card">
        <div class="card-head"><h2>DNS lookup</h2></div>
        <form class="dns-form" onsubmit={(e) => { e.preventDefault(); runLookup() }}>
          <input class="input" bind:value={lookupHost} placeholder="example.com" aria-label="Host to resolve" spellcheck="false" autocomplete="off" />
          <button class="btn" type="submit" disabled={lookupRunning}>{lookupRunning ? 'Resolving…' : 'Resolve'}</button>
        </form>
        {#if lookupResult}<ul class="dns-list mono">{#each lookupResult as ip (ip)}<li>{ip}</li>{/each}</ul>{/if}
        {#if lookupError}<p class="error-box">{lookupError}</p>{/if}
      </section>
    </div>

    <!-- Speed test -->
    <section class="card">
      <div class="card-head"><h2>Download speed</h2></div>
      <p class="dim small">Downloads ~8 MB from <code>speed.cloudflare.com/__down?bytes=8000000</code> to measure your link. Runs only when you click. Nothing else is contacted.</p>
      <button class="btn" onclick={runSpeed} disabled={speedRunning}>{speedRunning ? 'Downloading ~8 MB…' : 'Run speed test'}</button>
      {#if speed}
        <p class="result-line big">{formatMbps(speed.mbps)} <span class="dim small">({formatBytes(speed.bytes)} in {speed.seconds.toFixed(2)} s)</span></p>
      {/if}
      {#if speedError}<p class="error-box">{speedError}</p>{/if}
    </section>

    <!-- Latency history -->
    <section class="card">
      <div class="card-head"><h2>Latency history</h2>
        <button class="mini-btn" onclick={clearHistory}>clear</button>
      </div>
      {#if history.length === 0}
        <p class="dim small">No pings recorded yet. Run a ping to start the local history.</p>
      {:else}
        <div class="history">
          {#each history.slice().reverse() as entry, i (i)}
            <div class="history-row">
              <span class="history-ts dim mono">{new Date(entry.ts * 1000).toLocaleString()}</span>
              <span class="history-host mono">{entry.host}</span>
              <span class="history-bar"><span style:width="{((entry.avg_ms ?? 0) / maxHistory) * 100}%"></span></span>
              <span class="mono small">{entry.avg_ms == null ? '—' : entry.avg_ms.toFixed(1) + ' ms'}</span>
              <span class="mono small dim">{entry.loss_percent.toFixed(0)}% loss</span>
            </div>
          {/each}
        </div>
      {/if}
    </section>

    <!-- Report -->
    <section class="card">
      <div class="card-head"><h2>Diagnostic report</h2></div>
      <p class="dim small">Markdown export of everything shown above. Runs nothing new — unrun checks appear as “_not run_”. Saved to your Downloads folder.</p>
      <button class="btn" onclick={exportReport}>Download diagnostic report (.md)</button>
      {#if exportMsg}<p class="dim small">{exportMsg}</p>{/if}
    </section>
  </main>

  <footer class="statusbar">
    <span class="dim">NetPeek never scans your network or captures traffic. External contacts happen only on your command.</span>
    <span class="dim">local-only</span>
  </footer>

  {#if toast}<div class="toast">{toast}</div>{/if}
</div>

<style>
  .app { display: flex; flex-direction: column; height: 100%; }
  .topbar { display: flex; align-items: center; justify-content: space-between; padding: .55rem .95rem; background: var(--bg-elevated); border-bottom: 1px solid var(--border); user-select: none; }
  .brand { display: flex; align-items: center; gap: .55rem; }
  .logo { display: grid; place-items: center; width: 33px; height: 33px; border-radius: var(--radius-sm); background: var(--accent-soft); color: var(--accent); }
  h1 { font-size: .95rem; margin: 0; }
  .brand p { margin: 0; font-size: .68rem; color: var(--fg-dim); text-transform: uppercase; letter-spacing: .12em; }

  .scroll { flex: 1; overflow-y: auto; padding: .9rem 1rem 1.2rem; display: flex; flex-direction: column; gap: .75rem; }
  .card { background: var(--bg-elevated); border: 1px solid var(--border); border-radius: var(--radius); box-shadow: var(--shadow); padding: .7rem .85rem; }
  .card-head { display: flex; align-items: center; justify-content: space-between; gap: .6rem; margin-bottom: .4rem; }
  .card-head h2 { font-size: .88rem; margin: 0; font-weight: 650; }
  .small { font-size: .76rem; }
  .big { font-size: 1.05rem; }

  .kv-grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(220px, 1fr)); gap: .4rem .9rem; margin: .2rem 0 .5rem; }
  .kv { display: flex; flex-direction: column; }
  .k { font-size: .68rem; text-transform: uppercase; letter-spacing: .09em; color: var(--fg-dim); }
  .v { display: flex; align-items: center; gap: .4rem; font-size: .85rem; }
  .iface-table { border-top: 1px solid var(--border); padding-top: .4rem; }
  .iface-row { display: grid; grid-template-columns: 1fr 1fr auto; gap: .6rem; padding: .14rem .1rem; font-size: .8rem; border-bottom: 1px solid var(--border); }
  .iface-row:last-child { border-bottom: none; }
  .iface-head { font-size: .68rem; text-transform: uppercase; color: var(--fg-dim); letter-spacing: .09em; }

  .two-col { display: grid; grid-template-columns: 1fr 1fr; gap: .75rem; }
  .dot { display: inline-block; width: .65em; height: .65em; border-radius: 50%; background: var(--fg-dim); margin-right: .4em; }
  .dot.ok { background: var(--success); }
  .dot.bad { background: var(--danger); }
  .result-line { margin: .35rem 0 0; font-size: .85rem; }
  .mini-btn { font: inherit; font-size: .72rem; color: var(--accent); background: transparent; border: 1px solid var(--border); border-radius: var(--radius-sm); padding: .08rem .42rem; cursor: pointer; margin-left: .35rem; }
  .mini-btn:hover { border-color: var(--accent); }

  .ping-form, .dns-form { display: flex; gap: .4rem; }
  .ping-form .input { flex: 1; }
  .select { flex: 0 0 auto; width: auto; }
  .ping-result { margin-top: .55rem; }
  .ping-stats { display: flex; gap: 1.1rem; margin-bottom: .45rem; flex-wrap: wrap; }
  .stat { display: flex; flex-direction: column; align-items: center; }
  .num { font-size: 1.15rem; font-weight: 700; font-variant-numeric: tabular-nums; }
  .lbl { font-size: .64rem; text-transform: uppercase; letter-spacing: .09em; color: var(--fg-dim); }
  .reply-bars { display: flex; align-items: flex-end; gap: 3px; height: 56px; padding-top: .3rem; border-top: 1px solid var(--border); }
  .reply-bar { flex: 1; min-width: 8px; background: var(--accent); opacity: .75; border-radius: 2px 2px 0 0; min-height: 3px; }

  .dns-list { margin: .3rem 0 .1rem; padding-left: 1.1rem; font-size: .82rem; }
  .history { display: flex; flex-direction: column; gap: .18rem; }
  .history-row { display: grid; grid-template-columns: 11.2em 1fr 9em 4.5em 4em; gap: .55rem; align-items: center; font-size: .76rem; padding: .1rem .1rem; border-radius: var(--radius-sm); }
  .history-row:hover { background: var(--bg-hover); }
  .history-host { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .history-bar { height: .62em; background: var(--bg-inset); border-radius: 999px; overflow: hidden; }
  .history-bar > span { display: block; height: 100%; background: var(--accent); opacity: .8; }

  .statusbar { display: flex; justify-content: space-between; gap: 1rem; padding: .34rem .95rem; font-size: .7rem; background: var(--bg-elevated); border-top: 1px solid var(--border); user-select: none; }
  @media (max-width: 880px) {
    .two-col { grid-template-columns: 1fr; }
    .history-row { grid-template-columns: 1fr 1fr 5em; }
    .history-bar, .history-ts, .history-row .mono.small.dim { display: none; }
  }
</style>
