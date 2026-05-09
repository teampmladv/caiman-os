import { useState, useEffect, useRef } from 'react'
import { useApp } from '../store.jsx'
import { C, Card, Bar, StatusDot, Badge, Btn } from '../components/UI.jsx'
import { getActiveCluster } from '../components/clusters/ClusterStore.js'

// ── API helpers ───────────────────────────────────────────────────────────
async function apiCall(method, path, body) {
  const cluster = getActiveCluster()
  if (!cluster) throw new Error('No cluster connected')
  const res = await fetch(`${cluster.url}${path}`, {
    method,
    headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${cluster.token}` },
    body: body ? JSON.stringify(body) : undefined,
  })
  if (!res.ok) { const e = await res.json(); throw new Error(e.error || `HTTP ${res.status}`) }
  if (res.status === 204) return null
  return res.json()
}

// ── Status colors ─────────────────────────────────────────────────────────
const STATUS = {
  RUNNING: { color: '#22c55e', label: 'Running', dot: true  },
  BOOTING: { color: '#ffb800', label: 'Booting', dot: true  },
  STOPPED: { color: '#475569', label: 'Stopped', dot: false },
  ERROR:   { color: '#ff4466', label: 'Error',   dot: false },
}

// ── Create VM Wizard ──────────────────────────────────────────────────────
const TEMPLATES = [
  { id: 'micro',   label: 'Micro',      cpus: 1, mem: 512,  disk: 10,  icon: '🔹', desc: '1 vCPU · 512MB · 10GB' },
  { id: 'small',   label: 'Small',      cpus: 1, mem: 1024, disk: 20,  icon: '🟦', desc: '1 vCPU · 1GB · 20GB'  },
  { id: 'medium',  label: 'Medium',     cpus: 2, mem: 2048, disk: 40,  icon: '🟩', desc: '2 vCPU · 2GB · 40GB'  },
  { id: 'large',   label: 'Large',      cpus: 4, mem: 4096, disk: 80,  icon: '🟨', desc: '4 vCPU · 4GB · 80GB'  },
  { id: 'custom',  label: 'Custom',     cpus: 2, mem: 2048, disk: 40,  icon: '⚙️',  desc: 'Configure manually'   },
]

function CreateVMModal({ onClose, onCreated }) {
  const [step, setStep]   = useState(1) // 1=template 2=config 3=creating
  const [tpl, setTpl]     = useState(null)
  const [form, setForm]   = useState({ name: '', cpus: 2, mem: 2048, kernel: '/var/lib/caiman/kernels/vmlinuz', net_mode: 'nat' })
  const [error, setError] = useState('')

  const pick = (t) => {
    setTpl(t)
    setForm(f => ({ ...f, cpus: t.cpus, mem: t.mem }))
    if (t.id !== 'custom') setStep(2)
    else setStep(2)
  }

  const create = async () => {
    if (!form.name) { setError('Name is required'); return }
    setStep(3); setError('')
    try {
      const vm = await apiCall('POST', '/api/vms', {
        name:     form.name,
        cpus:     Number(form.cpus),
        memMib:   Number(form.mem),
        kernel:   form.kernel,
        netMode:  form.net_mode,
      })
      onCreated(vm)
      onClose()
    } catch(e) {
      setError(e.message)
      setStep(2)
    }
  }

  const inp = { background: '#080c12', border: `1px solid ${C.brd}`, color: C.txt, padding: '8px 10px', fontSize: 12, outline: 'none', fontFamily: 'IBM Plex Mono, monospace', width: '100%', boxSizing: 'border-box', borderRadius: 3 }

  return (
    <div style={{ position: 'fixed', inset: 0, zIndex: 1000, background: 'rgba(0,0,0,0.8)', backdropFilter: 'blur(4px)', display: 'flex', alignItems: 'center', justifyContent: 'center' }}
      onClick={e => e.target === e.currentTarget && onClose()}>
      <div style={{ background: '#0d1117', border: `1px solid ${C.brd}`, borderRadius: 10, width: 480, fontFamily: 'Syne, sans-serif', color: C.txt, boxShadow: '0 24px 80px #000a' }}>

        {/* Header */}
        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', padding: '18px 22px 14px', borderBottom: `1px solid ${C.brd}` }}>
          <div>
            <div style={{ fontSize: 16, fontWeight: 700 }}>Create Virtual Machine</div>
            <div style={{ fontSize: 11, color: C.dim, marginTop: 2 }}>
              Step {step} of 3 — {step === 1 ? 'Choose template' : step === 2 ? 'Configure' : 'Creating...'}
            </div>
          </div>
          <button onClick={onClose} style={{ background: 'none', border: 'none', color: C.dim, fontSize: 18, cursor: 'pointer' }}>x</button>
        </div>

        <div style={{ padding: '18px 22px' }}>
          {/* Step 1: Template */}
          {step === 1 && (
            <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
              {TEMPLATES.map(t => (
                <button key={t.id} onClick={() => pick(t)}
                  style={{ display: 'flex', alignItems: 'center', gap: 12, padding: '12px 14px', background: tpl?.id === t.id ? '#22c55e0a' : C.bg2, border: `1px solid ${tpl?.id === t.id ? C.g : C.brd}`, borderRadius: 6, cursor: 'pointer', textAlign: 'left', transition: 'all 0.15s' }}>
                  <span style={{ fontSize: 20 }}>{t.icon}</span>
                  <div style={{ flex: 1 }}>
                    <div style={{ fontSize: 13, color: C.txt, fontWeight: 600 }}>{t.label}</div>
                    <div style={{ fontSize: 11, color: C.dim, fontFamily: 'IBM Plex Mono, monospace' }}>{t.desc}</div>
                  </div>
                  {tpl?.id === t.id && <span style={{ color: C.g }}>✓</span>}
                </button>
              ))}
            </div>
          )}

          {/* Step 2: Config */}
          {step === 2 && (
            <div style={{ display: 'flex', flexDirection: 'column', gap: 14 }}>
              <div>
                <div style={{ fontSize: 10, color: C.dim, letterSpacing: '0.08em', textTransform: 'uppercase', marginBottom: 6 }}>VM Name</div>
                <input style={inp} placeholder="my-vm-01" value={form.name} onChange={e => setForm(f => ({ ...f, name: e.target.value }))} autoFocus />
              </div>
              <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 12 }}>
                <div>
                  <div style={{ fontSize: 10, color: C.dim, letterSpacing: '0.08em', textTransform: 'uppercase', marginBottom: 6 }}>vCPUs</div>
                  <input style={inp} type="number" min="1" max="32" value={form.cpus} onChange={e => setForm(f => ({ ...f, cpus: e.target.value }))} />
                </div>
                <div>
                  <div style={{ fontSize: 10, color: C.dim, letterSpacing: '0.08em', textTransform: 'uppercase', marginBottom: 6 }}>RAM (MiB)</div>
                  <input style={inp} type="number" min="256" step="256" value={form.mem} onChange={e => setForm(f => ({ ...f, mem: e.target.value }))} />
                </div>
              </div>
              <div>
                <div style={{ fontSize: 10, color: C.dim, letterSpacing: '0.08em', textTransform: 'uppercase', marginBottom: 6 }}>Kernel</div>
                <input style={inp} placeholder="/boot/vmlinuz" value={form.kernel} onChange={e => setForm(f => ({ ...f, kernel: e.target.value }))} />
              </div>
              <div>
                <div style={{ fontSize: 10, color: C.dim, letterSpacing: '0.08em', textTransform: 'uppercase', marginBottom: 6 }}>Network</div>
                <div style={{ display: 'flex', gap: 6 }}>
                  {['nat', 'bridge', 'none'].map(m => (
                    <button key={m} onClick={() => setForm(f => ({ ...f, net_mode: m }))}
                      style={{ flex: 1, padding: '7px 0', background: form.net_mode === m ? '#22c55e22' : 'transparent', border: `1px solid ${form.net_mode === m ? C.g : C.brd}`, color: form.net_mode === m ? C.g : C.dim, cursor: 'pointer', fontSize: 11, fontFamily: 'IBM Plex Mono, monospace', borderRadius: 3 }}>
                      {m}
                    </button>
                  ))}
                </div>
              </div>
              {error && <div style={{ background: '#ff446611', border: '1px solid #ff446633', padding: '8px 10px', fontSize: 11, color: '#ff4466', fontFamily: 'IBM Plex Mono, monospace' }}>{error}</div>}
            </div>
          )}

          {/* Step 3: Creating */}
          {step === 3 && (
            <div style={{ textAlign: 'center', padding: '32px 0' }}>
              <div style={{ fontSize: 32, marginBottom: 12, animation: 'pulse 1s infinite' }}>🐊</div>
              <div style={{ fontSize: 13, color: C.dim }}>Creating <strong style={{ color: C.txt }}>{form.name}</strong>...</div>
            </div>
          )}
        </div>

        {/* Footer */}
        {step !== 3 && (
          <div style={{ display: 'flex', gap: 8, justifyContent: 'flex-end', padding: '14px 22px', borderTop: `1px solid ${C.brd}` }}>
            <Btn onClick={step === 1 ? onClose : () => setStep(1)}>
              {step === 1 ? 'Cancel' : '← Back'}
            </Btn>
            {step === 1 && tpl && <Btn primary onClick={() => setStep(2)}>Configure →</Btn>}
            {step === 2 && <Btn primary onClick={create}>Create VM</Btn>}
          </div>
        )}
      </div>
    </div>
  )
}

// ── VM Detail Panel ───────────────────────────────────────────────────────
function VMDetail({ vm, onAction, onClose }) {
  const [tab, setTab]         = useState('overview') // overview | console | snapshots
  const [consoleLogs, setLogs] = useState([])
  const [loading, setLoading] = useState(false)
  const consoleRef            = useRef(null)
  const wsRef                 = useRef(null)

  const s = STATUS[vm.status] || STATUS.STOPPED

  // Fetch logs / open WebSocket for console
  useEffect(() => {
    if (tab !== 'console') return
    const cluster = getActiveCluster()
    if (!cluster) return

    // Try WebSocket first
    const wsUrl = `${cluster.url.replace('https', 'wss').replace('http', 'ws')}/api/vms/${vm.id}/console/ws`
    try {
      const ws = new WebSocket(wsUrl, [], { headers: { Authorization: `Bearer ${cluster.token}` } })
      wsRef.current = ws
      ws.onmessage = e => {
        setLogs(l => [...l.slice(-200), e.data])
        setTimeout(() => consoleRef.current?.scrollTo(0, 99999), 50)
      }
      ws.onerror = () => {
        // Fallback to HTTP logs
        fetch(`${cluster.url}/api/vms/${vm.id}/console`, {
          headers: { Authorization: `Bearer ${cluster.token}` }
        }).then(r => r.json()).then(lines => {
          setLogs(Array.isArray(lines) ? lines : [])
          setTimeout(() => consoleRef.current?.scrollTo(0, 99999), 50)
        }).catch(() => setLogs(['[console not available]']))
      }
    } catch(e) {
      setLogs(['[WebSocket not supported]'])
    }

    return () => { wsRef.current?.close() }
  }, [tab, vm.id])

  const action = async (type) => {
    setLoading(true)
    try { await onAction(type, vm.id) }
    finally { setLoading(false) }
  }

  const cpuPct  = vm.cpu_usage_pct  || vm.cpu  || 0
  const memUsed = vm.mem_used_mib   || 0
  const memTot  = vm.mem_mib        || vm.mem  || 256

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100%', background: C.bg2, borderLeft: `1px solid ${C.brd}` }}>
      {/* VM Header */}
      <div style={{ padding: '14px 16px', borderBottom: `1px solid ${C.brd}`, background: C.bg }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 10, marginBottom: 10 }}>
          <span style={{ width: 10, height: 10, borderRadius: '50%', background: s.color, boxShadow: s.dot ? `0 0 6px ${s.color}` : 'none', animation: s.dot ? 'pulse 2s infinite' : 'none', flexShrink: 0 }} />
          <div style={{ flex: 1, minWidth: 0 }}>
            <div style={{ fontSize: 15, fontWeight: 700, color: C.txt, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{vm.name}</div>
            <div style={{ fontSize: 10, color: C.dim, fontFamily: 'IBM Plex Mono, monospace' }}>{vm.id}</div>
          </div>
          <button onClick={onClose} style={{ background: 'none', border: 'none', color: C.dim, cursor: 'pointer', fontSize: 16, padding: 4 }}>✕</button>
        </div>

        {/* Action buttons */}
        <div style={{ display: 'flex', gap: 6 }}>
          {vm.status === 'RUNNING' || vm.status === 'BOOTING' ? (
            <>
              <button onClick={() => action('stop')} disabled={loading}
                style={{ flex: 1, background: '#ff446618', border: '1px solid #ff446633', color: '#ff4466', padding: '6px 0', cursor: 'pointer', fontSize: 11, fontFamily: 'IBM Plex Mono, monospace', borderRadius: 3 }}>
                ■ Stop
              </button>
              <button onClick={() => action('force-stop')} disabled={loading}
                style={{ background: 'transparent', border: `1px solid ${C.brd}`, color: C.dim, padding: '6px 10px', cursor: 'pointer', fontSize: 11, fontFamily: 'IBM Plex Mono, monospace', borderRadius: 3 }}>
                ⚡ Kill
              </button>
            </>
          ) : (
            <button onClick={() => action('start')} disabled={loading}
              style={{ flex: 1, background: '#22c55e18', border: '1px solid #22c55e33', color: '#22c55e', padding: '6px 0', cursor: 'pointer', fontSize: 11, fontFamily: 'IBM Plex Mono, monospace', borderRadius: 3 }}>
              ▶ Start
            </button>
          )}
          <button onClick={() => action('snapshot')} disabled={loading}
            style={{ background: 'transparent', border: `1px solid ${C.brd}`, color: C.dim, padding: '6px 10px', cursor: 'pointer', fontSize: 11, fontFamily: 'IBM Plex Mono, monospace', borderRadius: 3 }}>
            📸
          </button>
          <button onClick={() => action('delete')} disabled={loading}
            style={{ background: 'transparent', border: `1px solid ${C.brd}`, color: C.dim, padding: '6px 10px', cursor: 'pointer', fontSize: 11, fontFamily: 'IBM Plex Mono, monospace', borderRadius: 3 }}>
            🗑
          </button>
        </div>
      </div>

      {/* Tabs */}
      <div style={{ display: 'flex', borderBottom: `1px solid ${C.brd}`, flexShrink: 0 }}>
        {['overview', 'console', 'snapshots'].map(t => (
          <button key={t} onClick={() => setTab(t)}
            style={{ flex: 1, padding: '9px 0', background: 'none', border: 'none', borderBottom: tab === t ? `2px solid ${C.g}` : '2px solid transparent', color: tab === t ? C.g : C.dim, cursor: 'pointer', fontSize: 11, fontFamily: 'IBM Plex Mono, monospace', letterSpacing: '0.06em', textTransform: 'uppercase' }}>
            {t}
          </button>
        ))}
      </div>

      {/* Tab content */}
      <div style={{ flex: 1, overflow: 'auto' }}>

        {/* Overview */}
        {tab === 'overview' && (
          <div style={{ padding: 16, display: 'flex', flexDirection: 'column', gap: 14 }}>
            {/* Metrics */}
            <div>
              <div style={{ fontSize: 9, color: C.dim, letterSpacing: '0.1em', textTransform: 'uppercase', marginBottom: 8 }}>Resources</div>
              <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
                <div>
                  <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: 11, color: C.dim, marginBottom: 4 }}>
                    <span>CPU</span>
                    <span style={{ color: cpuPct > 80 ? C.red : cpuPct > 60 ? C.amb : C.g }}>{Math.round(cpuPct)}%</span>
                  </div>
                  <div style={{ height: 4, background: C.brd, borderRadius: 2 }}>
                    <div style={{ height: '100%', width: `${Math.min(cpuPct, 100)}%`, background: cpuPct > 80 ? C.red : cpuPct > 60 ? C.amb : C.g, borderRadius: 2, transition: 'width 0.8s' }} />
                  </div>
                </div>
                <div>
                  <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: 11, color: C.dim, marginBottom: 4 }}>
                    <span>RAM</span>
                    <span style={{ color: C.blu }}>{memUsed > 0 ? `${memUsed}/${memTot} MiB` : `${memTot} MiB`}</span>
                  </div>
                  <div style={{ height: 4, background: C.brd, borderRadius: 2 }}>
                    <div style={{ height: '100%', width: memUsed > 0 ? `${Math.min(memUsed/memTot*100, 100)}%` : '0%', background: C.blu, borderRadius: 2 }} />
                  </div>
                </div>
              </div>
            </div>

            {/* Info */}
            <div>
              <div style={{ fontSize: 9, color: C.dim, letterSpacing: '0.1em', textTransform: 'uppercase', marginBottom: 8 }}>Configuration</div>
              {[
                ['vCPUs',   vm.cpus],
                ['Memory',  `${vm.mem_mib || vm.mem} MiB`],
                ['Node',    vm.node_name || vm.node || '—'],
                ['Uplink',  vm.uplink || '—'],
                ['Kernel',  (vm.kernel || '—').split('/').pop()],
                ['Disk',    vm.disk || '—'],
                ['Created', vm.created_at ? new Date(vm.created_at).toLocaleDateString() : '—'],
              ].map(([k, v]) => (
                <div key={k} style={{ display: 'flex', justifyContent: 'space-between', padding: '5px 0', borderBottom: `1px solid ${C.brd}22`, fontSize: 11 }}>
                  <span style={{ color: C.dim }}>{k}</span>
                  <span style={{ color: C.txt, fontFamily: 'IBM Plex Mono, monospace', fontSize: 10 }}>{v}</span>
                </div>
              ))}
            </div>

            {/* Labels */}
            {vm.labels && Object.keys(vm.labels).length > 0 && (
              <div>
                <div style={{ fontSize: 9, color: C.dim, letterSpacing: '0.1em', textTransform: 'uppercase', marginBottom: 8 }}>Labels</div>
                <div style={{ display: 'flex', flexWrap: 'wrap', gap: 4 }}>
                  {Object.entries(vm.labels).map(([k, v]) => (
                    <span key={k} style={{ fontSize: 10, padding: '2px 7px', background: '#00d4ff11', color: '#00d4ff', border: '1px solid #00d4ff22', fontFamily: 'IBM Plex Mono, monospace' }}>
                      {k}={v}
                    </span>
                  ))}
                </div>
              </div>
            )}
          </div>
        )}

        {/* Console */}
        {tab === 'console' && (
          <div style={{ height: '100%', display: 'flex', flexDirection: 'column' }}>
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', padding: '8px 12px', borderBottom: `1px solid ${C.brd}`, flexShrink: 0 }}>
              <span style={{ fontSize: 10, color: C.dim, fontFamily: 'IBM Plex Mono, monospace' }}>Serial console — {vm.name}</span>
              <button onClick={() => setLogs([])} style={{ background: 'none', border: 'none', color: C.dim, cursor: 'pointer', fontSize: 10, fontFamily: 'IBM Plex Mono, monospace' }}>Clear</button>
            </div>
            <div ref={consoleRef} style={{ flex: 1, overflow: 'auto', padding: 12, fontFamily: 'IBM Plex Mono, monospace', fontSize: 11, lineHeight: 1.6, color: '#22c55e', background: '#020408' }}>
              {consoleLogs.length === 0
                ? <span style={{ color: '#334155' }}>[waiting for output...]</span>
                : consoleLogs.map((line, i) => <div key={i}>{line}</div>)
              }
            </div>
          </div>
        )}

        {/* Snapshots */}
        {tab === 'snapshots' && (
          <div style={{ padding: 16 }}>
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 12 }}>
              <div style={{ fontSize: 11, color: C.dim }}>Snapshots</div>
              <button onClick={() => action('snapshot')}
                style={{ background: '#22c55e18', border: '1px solid #22c55e33', color: C.g, padding: '5px 12px', cursor: 'pointer', fontSize: 11, fontFamily: 'IBM Plex Mono, monospace', borderRadius: 3 }}>
                + Snapshot
              </button>
            </div>
            <div style={{ fontSize: 11, color: C.dim, textAlign: 'center', padding: 24 }}>No snapshots yet</div>
          </div>
        )}
      </div>
    </div>
  )
}

// ── Main VMs view ─────────────────────────────────────────────────────────
export default function VMs() {
  const { vms: mockVms, stopVM: mockStop, startVM: mockStart, deleteVM: mockDelete, createSnapshot, showToast } = useApp()
  const [vms, setVms]           = useState([])
  const [selected, setSelected] = useState(null)
  const [search, setSearch]     = useState('')
  const [filter, setFilter]     = useState('')
  const [showCreate, setCreate] = useState(false)
  const [loading, setLoading]   = useState(false)
  const cluster = getActiveCluster()

  // Load VMs from real API or fallback to mock
  const loadVMs = async () => {
    if (!cluster) { setVms(mockVms); return }
    try {
      const data = await apiCall('GET', '/api/vms')
      setVms(Array.isArray(data) ? data : [])
    } catch(e) {
      setVms(mockVms)
    }
  }

  useEffect(() => {
    loadVMs()
    const interval = setInterval(loadVMs, 5000)
    const handler = () => loadVMs()
    window.addEventListener('caiman:cluster-changed', handler)
    return () => { clearInterval(interval); window.removeEventListener('caiman:cluster-changed', handler) }
  }, [cluster?.id])

  // Update selected VM when list refreshes
  useEffect(() => {
    if (selected) {
      const updated = vms.find(v => v.id === selected.id)
      if (updated) setSelected(updated)
    }
  }, [vms])

  const filtered = vms.filter(v => {
    if (filter && v.status !== filter) return false
    const name = v.name || ''
    if (search && !name.toLowerCase().includes(search.toLowerCase())) return false
    return true
  })

  const handleAction = async (type, vmId) => {
    const vm = vms.find(v => v.id === vmId)
    if (!cluster) {
      // Mock fallback
      if (type === 'stop')   mockStop(vmId)
      if (type === 'start')  mockStart(vmId)
      if (type === 'delete') { mockDelete(vmId); setSelected(null) }
      if (type === 'snapshot') createSnapshot(vmId)
      return
    }
    try {
      if (type === 'stop')        await apiCall('POST', `/api/vms/${vmId}/stop`)
      if (type === 'force-stop')  await apiCall('POST', `/api/vms/${vmId}/force-stop`)
      if (type === 'start')       await apiCall('POST', `/api/vms/${vmId}/start`)
      if (type === 'delete') {
        await apiCall('DELETE', `/api/vms/${vmId}`)
        setSelected(null)
      }
      showToast(`VM ${type} successful`)
      await loadVMs()
    } catch(e) {
      showToast(`Error: ${e.message}`, 'error')
    }
  }

  const onCreated = (vm) => {
    showToast(`VM ${vm.name} created`)
    loadVMs()
    setSelected(vm)
  }

  const running = vms.filter(v => v.status === 'RUNNING' || v.status === 'running').length

  return (
    <div style={{ display: 'flex', height: '100%', overflow: 'hidden' }}>

      {/* ── Left panel: VM list ── */}
      <div style={{ width: selected ? 280 : '100%', minWidth: 240, display: 'flex', flexDirection: 'column', borderRight: selected ? `1px solid ${C.brd}` : 'none', transition: 'width 0.2s' }}>

        {/* Toolbar */}
        <div style={{ display: 'flex', alignItems: 'center', gap: 6, padding: '10px 12px', borderBottom: `1px solid ${C.brd}`, flexShrink: 0, background: C.bg }}>
          <input value={search} onChange={e => setSearch(e.target.value)}
            placeholder="Search..." style={{ background: C.bg3, border: `1px solid ${C.brd}`, color: C.txt, padding: '5px 9px', fontFamily: 'IBM Plex Mono, monospace', fontSize: 11, width: 120, outline: 'none', borderRadius: 3 }} />
          <div style={{ flex: 1 }} />
          <div style={{ fontSize: 10, color: C.dim, fontFamily: 'IBM Plex Mono, monospace' }}>{running}/{vms.length}</div>
          <button onClick={() => setCreate(true)}
            style={{ background: C.g, border: 'none', color: '#000', padding: '6px 12px', cursor: 'pointer', fontSize: 11, fontWeight: 700, fontFamily: 'IBM Plex Mono, monospace', borderRadius: 3 }}>
            + New
          </button>
        </div>

        {/* Filter tabs */}
        <div style={{ display: 'flex', borderBottom: `1px solid ${C.brd}`, flexShrink: 0 }}>
          {[['', 'All'], ['RUNNING', 'Running'], ['STOPPED', 'Stopped']].map(([val, label]) => (
            <button key={val} onClick={() => setFilter(val)}
              style={{ flex: 1, padding: '7px 0', background: 'none', border: 'none', borderBottom: filter === val ? `2px solid ${C.g}` : '2px solid transparent', color: filter === val ? C.g : C.dim, cursor: 'pointer', fontSize: 10, fontFamily: 'IBM Plex Mono, monospace', letterSpacing: '0.04em' }}>
              {label}
            </button>
          ))}
        </div>

        {/* VM list */}
        <div style={{ flex: 1, overflowY: 'auto' }}>
          {filtered.length === 0 && (
            <div style={{ textAlign: 'center', padding: 32, color: C.dim, fontSize: 12 }}>
              {vms.length === 0 ? (
                <div>
                  <div style={{ fontSize: 28, marginBottom: 8 }}>🐊</div>
                  <div>No VMs yet</div>
                  <div style={{ fontSize: 11, marginTop: 4 }}>Click + New to create one</div>
                </div>
              ) : 'No VMs match filter'}
            </div>
          )}

          {filtered.map(vm => {
            const s = STATUS[vm.status?.toUpperCase()] || STATUS.STOPPED
            const isSelected = selected?.id === vm.id
            const cpuPct = vm.cpu_usage_pct || vm.cpu || 0

            return (
              <div key={vm.id} onClick={() => setSelected(isSelected ? null : vm)}
                style={{ display: 'flex', alignItems: 'center', gap: 10, padding: '10px 12px', cursor: 'pointer', background: isSelected ? `${C.g}0a` : 'transparent', borderLeft: `3px solid ${isSelected ? C.g : 'transparent'}`, borderBottom: `1px solid ${C.brd}`, transition: 'all 0.1s' }}>

                {/* Status dot */}
                <span style={{ width: 8, height: 8, borderRadius: '50%', background: s.color, boxShadow: s.dot ? `0 0 5px ${s.color}` : 'none', flexShrink: 0, animation: s.dot ? 'pulse 2s infinite' : 'none' }} />

                {/* Info */}
                <div style={{ flex: 1, minWidth: 0 }}>
                  <div style={{ fontSize: 12, color: C.txt, fontWeight: 600, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{vm.name}</div>
                  <div style={{ fontSize: 10, color: C.dim, fontFamily: 'IBM Plex Mono, monospace', marginTop: 2 }}>
                    {vm.cpus}vCPU · {vm.mem_mib || vm.mem}M
                  </div>
                  {/* Mini CPU bar */}
                  {cpuPct > 0 && (
                    <div style={{ height: 2, background: C.brd, borderRadius: 1, marginTop: 4 }}>
                      <div style={{ height: '100%', width: `${Math.min(cpuPct, 100)}%`, background: cpuPct > 80 ? C.red : cpuPct > 60 ? C.amb : C.g, borderRadius: 1, transition: 'width 1s' }} />
                    </div>
                  )}
                </div>

                {/* CPU % */}
                {cpuPct > 0 && (
                  <span style={{ fontSize: 10, color: cpuPct > 80 ? C.red : C.dim, fontFamily: 'IBM Plex Mono, monospace', flexShrink: 0 }}>
                    {Math.round(cpuPct)}%
                  </span>
                )}
              </div>
            )
          })}
        </div>
      </div>

      {/* ── Right panel: VM detail ── */}
      {selected && (
        <div style={{ flex: 1, minWidth: 0, overflow: 'hidden' }}>
          <VMDetail vm={selected} onAction={handleAction} onClose={() => setSelected(null)} />
        </div>
      )}

      {/* ── Create VM modal ── */}
      {showCreate && <CreateVMModal onClose={() => setCreate(false)} onCreated={onCreated} />}

      <style>{`
        @keyframes pulse { 0%,100%{opacity:1} 50%{opacity:0.3} }
      `}</style>
    </div>
  )
}
