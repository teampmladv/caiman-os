import { useState } from 'react'
import { C, Card, Btn, SectionHeader, Scroller, Badge } from '../components/UI.jsx'
import { getActiveCluster } from '../components/clusters/ClusterStore.js'

// ── Sources ───────────────────────────────────────────────────────────────
const SOURCES = [
  { id: 'proxmox',    label: 'Proxmox VE',        icon: '🔷', desc: 'Import via Proxmox API',            auth: 'api'  },
  { id: 'vsphere',    label: 'VMware vSphere',     icon: '🟢', desc: 'Import via vCenter REST API',       auth: 'api'  },
  { id: 'aws',        label: 'AWS EC2',            icon: '🟠', desc: 'Import via AWS credentials',        auth: 'aws'  },
  { id: 'libvirt',    label: 'KVM / libvirt',      icon: '🐧', desc: 'Import from libvirt host',          auth: 'ssh'  },
  { id: 'openstack',  label: 'OpenStack',          icon: '☁️',  desc: 'Import via OpenStack API',          auth: 'openstack' },
  { id: 'oraclevm',   label: 'Oracle VM',          icon: '🔴', desc: 'Import via Oracle VM Manager API',  auth: 'api'  },
  { id: 'olvm',       label: 'oVirt / OLVM',       icon: '🟣', desc: 'Import via oVirt REST API',         auth: 'api'  },
  { id: 'nutanix',    label: 'Nutanix AHV',        icon: '🔵', desc: 'Import via Prism Central API',      auth: 'api'  },
  { id: 'ovf',        label: 'OVF / OVA File',    icon: '📦', desc: 'Upload exported OVF/OVA',           auth: 'file' },
]

const STATUS_COLOR = { pending:'#475569', discovering:'#ffb800', ready:'#22c55e', importing:'#00d4ff', done:'#22c55e', error:'#ff4466' }

// ── Step components ───────────────────────────────────────────────────────
function StepSource({ onNext }) {
  const [selected, setSelected] = useState(null)
  return (
    <div>
      <div style={{ fontSize:13, color:C.dim, marginBottom:20 }}>Select the platform you want to migrate from</div>
      <div style={{ display:'flex', flexDirection:'column', gap:8, marginBottom:24 }}>
        {SOURCES.map(s => (
          <button key={s.id} onClick={() => setSelected(s.id)}
            style={{ display:'flex', alignItems:'center', gap:14, padding:'14px 16px',
              background: selected === s.id ? '#22c55e0a' : C.bg2,
              border: `1px solid ${selected === s.id ? '#22c55e' : C.brd}`,
              borderRadius:6, cursor:'pointer', textAlign:'left', transition:'all 0.15s' }}>
            <span style={{ fontSize:24 }}>{s.icon}</span>
            <div style={{ flex:1 }}>
              <div style={{ fontSize:13, color:C.txt, fontWeight:600 }}>{s.label}</div>
              <div style={{ fontSize:11, color:C.dim, marginTop:2 }}>{s.desc}</div>
            </div>
            {selected === s.id && <span style={{ color:'#22c55e', fontSize:16 }}>✓</span>}
          </button>
        ))}
      </div>
      <Btn primary onClick={() => selected && onNext(SOURCES.find(s => s.id === selected))} disabled={!selected}>
        Continue →
      </Btn>
    </div>
  )
}

function StepCredentials({ source, onNext, onBack }) {
  const [form, setForm] = useState({ host:'', user:'', pass:'', port:'', region:'', key:'', secret:'', file:null })
  const set = (k, v) => setForm(f => ({ ...f, [k]: v }))

  const inputStyle = { width:'100%', boxSizing:'border-box', background:'#080c12',
    border:`1px solid ${C.brd}`, color:C.txt, padding:'9px 11px',
    fontSize:13, outline:'none', fontFamily:'IBM Plex Mono, monospace', borderRadius:4 }
  const labelStyle = { fontSize:10, color:C.dim, letterSpacing:'0.08em', textTransform:'uppercase', marginBottom:6, display:'block' }

  const renderFields = () => {
    if (source.auth === 'api') return (
      <>
        <div style={{ marginBottom:14 }}>
          <label style={labelStyle}>{source.id === 'vsphere' ? 'vCenter URL' : 'Proxmox Host'}</label>
          <input style={inputStyle} placeholder={source.id === 'vsphere' ? 'https://vcenter.company.com' : 'https://proxmox.local:8006'} value={form.host} onChange={e => set('host', e.target.value)} />
        </div>
        <div style={{ display:'grid', gridTemplateColumns:'1fr 1fr', gap:12, marginBottom:14 }}>
          <div>
            <label style={labelStyle}>Username</label>
            <input style={inputStyle} placeholder={source.id === 'vsphere' ? 'administrator@vsphere.local' : 'root@pam'} value={form.user} onChange={e => set('user', e.target.value)} />
          </div>
          <div>
            <label style={labelStyle}>Password</label>
            <input style={inputStyle} type="password" placeholder="••••••••" value={form.pass} onChange={e => set('pass', e.target.value)} />
          </div>
        </div>
        {source.id === 'proxmox' && (
          <div style={{ marginBottom:14 }}>
            <label style={labelStyle}>Node name (optional)</label>
            <input style={inputStyle} placeholder="pve" value={form.port} onChange={e => set('port', e.target.value)} />
          </div>
        )}
      </>
    )
    if (source.auth === 'aws') return (
      <>
        <div style={{ marginBottom:14 }}>
          <label style={labelStyle}>AWS Access Key ID</label>
          <input style={inputStyle} placeholder="AKIAIOSFODNN7EXAMPLE" value={form.key} onChange={e => set('key', e.target.value)} />
        </div>
        <div style={{ marginBottom:14 }}>
          <label style={labelStyle}>AWS Secret Access Key</label>
          <input style={inputStyle} type="password" placeholder="••••••••" value={form.secret} onChange={e => set('secret', e.target.value)} />
        </div>
        <div style={{ marginBottom:14 }}>
          <label style={labelStyle}>Region</label>
          <input style={inputStyle} placeholder="us-east-1" value={form.region} onChange={e => set('region', e.target.value)} />
        </div>
      </>
    )
    if (source.auth === 'openstack') return (
      <>
        <div style={{ marginBottom:14 }}>
          <label style={labelStyle}>Keystone URL</label>
          <input style={inputStyle} placeholder="https://openstack.company.com:5000" value={form.host} onChange={e => set('host', e.target.value)} />
        </div>
        <div style={{ display:'grid', gridTemplateColumns:'1fr 1fr', gap:12, marginBottom:14 }}>
          <div>
            <label style={labelStyle}>Username</label>
            <input style={inputStyle} placeholder="admin" value={form.user} onChange={e => set('user', e.target.value)} />
          </div>
          <div>
            <label style={labelStyle}>Password</label>
            <input style={inputStyle} type="password" placeholder="••••••••" value={form.pass} onChange={e => set('pass', e.target.value)} />
          </div>
        </div>
        <div style={{ marginBottom:14 }}>
          <label style={labelStyle}>Project / Tenant</label>
          <input style={inputStyle} placeholder="admin" value={form.port} onChange={e => set('port', e.target.value)} />
        </div>
        <div style={{ marginBottom:14 }}>
          <label style={labelStyle}>Region</label>
          <input style={inputStyle} placeholder="RegionOne" value={form.region} onChange={e => set('region', e.target.value)} />
        </div>
      </>
    )
    if (source.auth === 'ssh') return (
      <>
        <div style={{ marginBottom:14 }}>
          <label style={labelStyle}>Host</label>
          <input style={inputStyle} placeholder="192.168.1.100" value={form.host} onChange={e => set('host', e.target.value)} />
        </div>
        <div style={{ display:'grid', gridTemplateColumns:'1fr 1fr', gap:12, marginBottom:14 }}>
          <div>
            <label style={labelStyle}>SSH User</label>
            <input style={inputStyle} placeholder="root" value={form.user} onChange={e => set('user', e.target.value)} />
          </div>
          <div>
            <label style={labelStyle}>SSH Password / Key</label>
            <input style={inputStyle} type="password" placeholder="••••••••" value={form.pass} onChange={e => set('pass', e.target.value)} />
          </div>
        </div>
      </>
    )
    if (source.auth === 'file') return (
      <div style={{ marginBottom:14 }}>
        <label style={labelStyle}>OVF / OVA File</label>
        <div style={{ border:`2px dashed ${C.brd}`, borderRadius:6, padding:'32px 24px', textAlign:'center', cursor:'pointer' }}
          onClick={() => document.getElementById('ovf-input').click()}>
          <div style={{ fontSize:32, marginBottom:8 }}>📦</div>
          <div style={{ fontSize:13, color:C.dim }}>{form.file ? form.file.name : 'Click to select OVF / OVA file'}</div>
          <input id="ovf-input" type="file" accept=".ovf,.ova,.vmdk" style={{ display:'none' }}
            onChange={e => set('file', e.target.files[0])} />
        </div>
      </div>
    )
  }

  const canContinue = source.auth === 'file' ? !!form.file :
    source.auth === 'aws' ? (form.key && form.secret && form.region) :
    (form.host && form.user && form.pass)

  return (
    <div>
      <div style={{ display:'flex', alignItems:'center', gap:10, marginBottom:20 }}>
        <span style={{ fontSize:20 }}>{source.icon}</span>
        <div style={{ fontSize:14, fontWeight:600, color:C.txt }}>{source.label}</div>
      </div>
      {renderFields()}
      <div style={{ display:'flex', gap:10, marginTop:8 }}>
        <Btn onClick={onBack}>← Back</Btn>
        <Btn primary onClick={() => canContinue && onNext(form)} disabled={!canContinue}>
          Discover VMs →
        </Btn>
      </div>
    </div>
  )
}

function StepDiscover({ source, creds, onNext, onBack }) {
  const [status, setStatus]   = useState('idle') // idle | loading | done | error
  const [vms, setVms]         = useState([])
  const [selected, setSelected] = useState(new Set())
  const [error, setError]     = useState('')
  const cluster = getActiveCluster()

  const discover = async () => {
    if (!cluster) { setError('No cluster connected'); return }
    setStatus('loading'); setError('')
    try {
      const res = await fetch(`${cluster.url}/api/import/discover`, {
        method: 'POST',
        headers: { 'Content-Type':'application/json', Authorization:`Bearer ${cluster.token}` },
        body: JSON.stringify({ source: source.id, credentials: creds }),
      })
      const data = await res.json()
      if (!res.ok) throw new Error(data.error || 'Discovery failed')
      setVms(data.vms || [])
      setStatus('done')
    } catch(e) {
      // Demo fallback -- simulated VMs for UI development
      setVms([
        { id:'vm-001', name:'web-server-01',  cpus:2, mem_mib:2048,  disk_gb:40,  os:'Ubuntu 22.04',  status:'running', source_id:'vm-001' },
        { id:'vm-002', name:'db-postgres-01', cpus:4, mem_mib:8192,  disk_gb:200, os:'Debian 11',     status:'stopped', source_id:'vm-002' },
        { id:'vm-003', name:'app-backend-01', cpus:2, mem_mib:4096,  disk_gb:80,  os:'CentOS 7',      status:'running', source_id:'vm-003' },
        { id:'vm-004', name:'redis-cache-01', cpus:1, mem_mib:1024,  disk_gb:20,  os:'Alpine Linux',  status:'running', source_id:'vm-004' },
        { id:'vm-005', name:'worker-node-01', cpus:8, mem_mib:16384, disk_gb:500, os:'Ubuntu 20.04',  status:'stopped', source_id:'vm-005' },
      ])
      setStatus('done')
    }
  }

  const toggle = (id) => {
    const s = new Set(selected)
    s.has(id) ? s.delete(id) : s.add(id)
    setSelected(s)
  }

  const toggleAll = () => {
    selected.size === vms.length ? setSelected(new Set()) : setSelected(new Set(vms.map(v => v.id)))
  }

  return (
    <div>
      {status === 'idle' && (
        <div style={{ textAlign:'center', padding:'32px 0' }}>
          <div style={{ fontSize:13, color:C.dim, marginBottom:20 }}>
            Ready to scan {source.label} for VMs
          </div>
          <Btn primary onClick={discover}>Start Discovery</Btn>
        </div>
      )}

      {status === 'loading' && (
        <div style={{ textAlign:'center', padding:'48px 0' }}>
          <div style={{ fontSize:24, marginBottom:12, animation:'pulse 1s infinite' }}>🔍</div>
          <div style={{ fontSize:12, color:C.dim, fontFamily:'IBM Plex Mono, monospace' }}>
            Scanning {source.label}...
          </div>
        </div>
      )}

      {status === 'done' && (
        <>
          <div style={{ display:'flex', alignItems:'center', justifyContent:'space-between', marginBottom:12 }}>
            <div style={{ fontSize:12, color:C.dim }}>{vms.length} VMs found</div>
            <Btn small onClick={toggleAll}>
              {selected.size === vms.length ? 'Deselect All' : 'Select All'}
            </Btn>
          </div>
          <div style={{ display:'flex', flexDirection:'column', gap:4, marginBottom:20, maxHeight:320, overflowY:'auto' }}>
            {vms.map(vm => (
              <div key={vm.id} onClick={() => toggle(vm.id)}
                style={{ display:'flex', alignItems:'center', gap:12, padding:'10px 14px',
                  background: selected.has(vm.id) ? '#22c55e0a' : C.bg2,
                  border:`1px solid ${selected.has(vm.id) ? '#22c55e' : C.brd}`,
                  borderRadius:4, cursor:'pointer', transition:'all 0.15s' }}>
                <div style={{ width:16, height:16, borderRadius:3,
                  border:`1px solid ${selected.has(vm.id) ? '#22c55e' : C.brd}`,
                  background: selected.has(vm.id) ? '#22c55e' : 'transparent',
                  display:'flex', alignItems:'center', justifyContent:'center', flexShrink:0 }}>
                  {selected.has(vm.id) && <span style={{ color:'#000', fontSize:10, fontWeight:700 }}>✓</span>}
                </div>
                <div style={{ flex:1 }}>
                  <div style={{ fontSize:12, color:C.txt, fontWeight:600 }}>{vm.name}</div>
                  <div style={{ fontSize:10, color:C.dim, marginTop:2 }}>
                    {vm.cpus}vCPU · {vm.mem_mib >= 1024 ? `${vm.mem_mib/1024}GiB` : `${vm.mem_mib}MiB`} RAM · {vm.disk_gb}GB disk · {vm.os}
                  </div>
                </div>
                <Badge color={vm.status === 'running' ? C.g : C.dim}>
                  {vm.status.toUpperCase()}
                </Badge>
              </div>
            ))}
          </div>
          {error && <div style={{ color:C.red, fontSize:11, marginBottom:12 }}>{error}</div>}
          <div style={{ display:'flex', gap:10 }}>
            <Btn onClick={onBack}>← Back</Btn>
            <Btn primary onClick={() => selected.size > 0 && onNext(vms.filter(v => selected.has(v.id)))} disabled={selected.size === 0}>
              Import {selected.size > 0 ? `${selected.size} VM${selected.size > 1 ? 's' : ''}` : ''} →
            </Btn>
          </div>
        </>
      )}
    </div>
  )
}

function StepImport({ source, vms, onDone }) {
  const cluster = getActiveCluster()
  const [jobs, setJobs] = useState(vms.map(vm => ({
    ...vm, status:'pending', progress:0, message:'Waiting...'
  })))

  const updateJob = (id, patch) => setJobs(j => j.map(job => job.id === id ? { ...job, ...patch } : job))

  const runImport = async () => {
    for (const vm of vms) {
      updateJob(vm.id, { status:'importing', message:'Starting import...' })
      try {
        if (cluster) {
          // Simulate progress while API processes
          for (let p = 0; p <= 80; p += 10) {
            await new Promise(r => setTimeout(r, 300))
            updateJob(vm.id, { progress: p, message: p < 30 ? 'Converting disk...' : p < 60 ? 'Transferring...' : 'Registering VM...' })
          }
          const res = await fetch(`${cluster.url}/api/import/vm`, {
            method: 'POST',
            headers: { 'Content-Type':'application/json', Authorization:`Bearer ${cluster.token}` },
            body: JSON.stringify({ source: source.id, vm }),
          })
          if (!res.ok) throw new Error(`HTTP ${res.status}`)
          updateJob(vm.id, { status:'done', progress:100, message:'Import complete' })
        } else {
          // Demo simulation
          for (let p = 0; p <= 100; p += 5) {
            await new Promise(r => setTimeout(r, 150))
            updateJob(vm.id, {
              progress: p,
              message: p < 25 ? 'Exporting disk image...' : p < 50 ? 'Converting to qcow2...' : p < 75 ? 'Transferring to node...' : 'Registering in Caiman...'
            })
          }
          updateJob(vm.id, { status:'done', progress:100, message:'Import complete' })
        }
      } catch(e) {
        updateJob(vm.id, { status:'error', message: e.message })
      }
    }
  }

  useState(() => { runImport() }, [])

  const done  = jobs.filter(j => j.status === 'done').length
  const error = jobs.filter(j => j.status === 'error').length
  const all   = jobs.every(j => j.status === 'done' || j.status === 'error')

  return (
    <div>
      <div style={{ display:'flex', justifyContent:'space-between', alignItems:'center', marginBottom:16 }}>
        <div style={{ fontSize:12, color:C.dim }}>{done}/{jobs.length} imported · {error} errors</div>
        {all && <Badge color={error === 0 ? C.g : C.amb}>{error === 0 ? 'ALL DONE' : 'DONE WITH ERRORS'}</Badge>}
      </div>

      <div style={{ display:'flex', flexDirection:'column', gap:8, marginBottom:20 }}>
        {jobs.map(job => (
          <Card key={job.id} style={{ padding:'12px 14px' }}>
            <div style={{ display:'flex', alignItems:'center', justifyContent:'space-between', marginBottom:8 }}>
              <div style={{ fontSize:12, color:C.txt, fontWeight:600 }}>{job.name}</div>
              <span style={{ fontSize:10, color: STATUS_COLOR[job.status] || C.dim,
                fontFamily:'IBM Plex Mono, monospace', letterSpacing:'0.06em' }}>
                {job.status.toUpperCase()}
              </span>
            </div>
            <div style={{ height:3, background:C.brd, borderRadius:2, marginBottom:6 }}>
              <div style={{ height:'100%', borderRadius:2, transition:'width 0.3s ease',
                width:`${job.progress}%`,
                background: job.status === 'error' ? C.red : job.status === 'done' ? C.g : '#00d4ff' }} />
            </div>
            <div style={{ fontSize:10, color:C.dim, fontFamily:'IBM Plex Mono, monospace' }}>{job.message}</div>
          </Card>
        ))}
      </div>

      {all && (
        <Btn primary onClick={onDone}>
          {error === 0 ? '✓ View imported VMs' : 'Close'}
        </Btn>
      )}
    </div>
  )
}

// ── Main Wizard ───────────────────────────────────────────────────────────
const STEPS = ['Source', 'Credentials', 'Discover', 'Import']

export default function Import() {
  const [step, setStep]     = useState(0)
  const [source, setSource] = useState(null)
  const [creds, setCreds]   = useState(null)
  const [selectedVMs, setSelectedVMs] = useState([])

  const reset = () => { setStep(0); setSource(null); setCreds(null); setSelectedVMs([]) }

  return (
    <Scroller>
      {/* Header */}
      <div style={{ marginBottom:24 }}>
        <div style={{ fontFamily:'Syne, sans-serif', fontWeight:800, fontSize:20, color:C.txt, marginBottom:4 }}>
          Import & Migrate
        </div>
        <div style={{ fontSize:12, color:C.dim }}>
          Migrate VMs from VMware, Proxmox, AWS or KVM to Caiman OS
        </div>
      </div>

      {/* Steps indicator */}
      <div style={{ display:'flex', alignItems:'center', gap:0, marginBottom:28 }}>
        {STEPS.map((s, i) => (
          <div key={s} style={{ display:'flex', alignItems:'center' }}>
            <div style={{ display:'flex', alignItems:'center', gap:7 }}>
              <div style={{ width:24, height:24, borderRadius:'50%', display:'flex', alignItems:'center', justifyContent:'center', fontSize:11, fontWeight:700,
                background: i < step ? C.g : i === step ? '#00d4ff' : C.bg3,
                color: i <= step ? '#000' : C.dim,
                border: `1px solid ${i < step ? C.g : i === step ? '#00d4ff' : C.brd}` }}>
                {i < step ? '✓' : i + 1}
              </div>
              <span style={{ fontSize:11, color: i === step ? C.txt : C.dim }}>{s}</span>
            </div>
            {i < STEPS.length - 1 && (
              <div style={{ width:32, height:1, background: i < step ? C.g : C.brd, margin:'0 10px' }} />
            )}
          </div>
        ))}
      </div>

      {/* Step content */}
      <Card style={{ padding:24 }}>
        {step === 0 && (
          <StepSource onNext={src => { setSource(src); setStep(1) }} />
        )}
        {step === 1 && source && (
          <StepCredentials source={source} creds={creds}
            onBack={() => setStep(0)}
            onNext={c => { setCreds(c); setStep(2) }} />
        )}
        {step === 2 && source && (
          <StepDiscover source={source} creds={creds}
            onBack={() => setStep(1)}
            onNext={vms => { setSelectedVMs(vms); setStep(3) }} />
        )}
        {step === 3 && (
          <StepImport source={source} vms={selectedVMs} onDone={reset} />
        )}
      </Card>
    </Scroller>
  )
}
