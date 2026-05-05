import { useState, useEffect, useRef } from 'react'
import { C, Btn } from '../components/UI.jsx'

const TEMPLATES = [
  {l:'INFO', s:'caiman-vmm', m:'vCPU 0 entering run loop'},
  {l:'INFO', s:'caiman-vmm', m:'virtio-net dataplane running on TAP tap0'},
  {l:'INFO', s:'caiman-api', m:'VM nginx-prod RUNNING pid=31337'},
  {l:'WARN', s:'caiman-vmm', m:'TAP interface tap0 already in use, retrying'},
  {l:'INFO', s:'caiman-drs', m:'DRS: migrating worker-01 to caiman-node-02'},
  {l:'INFO', s:'caiman-storage', m:'Snapshot postgres-snap-001 created (2.1GB)'},
  {l:'ERR',  s:'kernel', m:"i8042: Can't read CTR while initializing"},
  {l:'INFO', s:'caiman-net', m:'XDP program attached latency=7us ifindex=2'},
  {l:'INFO', s:'caiman-vmm', m:'KVM_SET_TSS_ADDRESS=0xfffbd000 AMD compat'},
  {l:'WARN', s:'caiman-drs', m:'CPU spike on caiman-node-03: 82%'},
  {l:'INFO', s:'kernel', m:'Memory: 2018144K/2096760K available'},
  {l:'INFO', s:'kernel', m:'serial8250: ttyS0 at I/O 0x3f8 irq=4 base_baud=115200'},
  {l:'INFO', s:'caiman-api', m:'POST /api/vms 201 in 4ms'},
  {l:'INFO', s:'caiman-bts', m:'Live migration complete: blackout=148ms'},
]

const LC = { INFO: C.blu, WARN: C.amb, ERR: C.red, OK: C.g }

export default function Logs() {
  const [logs, setLogs] = useState([])
  const [lvlFilter, setLvlFilter] = useState('')
  const [search, setSearch] = useState('')
  const [paused, setPaused] = useState(false)
  const bodyRef = useRef()

  useEffect(() => {
    const add = () => {
      if (paused) return
      const t = TEMPLATES[Math.floor(Math.random() * TEMPLATES.length)]
      const ts = new Date().toTimeString().slice(0, 8)
      setLogs(prev => [...prev.slice(-300), { ...t, ts, id: Math.random() }])
    }
    const initial = Array.from({ length: 25 }, () => {
      const t = TEMPLATES[Math.floor(Math.random() * TEMPLATES.length)]
      return { ...t, ts: new Date().toTimeString().slice(0, 8), id: Math.random() }
    })
    setLogs(initial)
    const interval = setInterval(add, 1200)
    return () => clearInterval(interval)
  }, [paused])

  useEffect(() => {
    if (!paused && bodyRef.current) bodyRef.current.scrollTop = bodyRef.current.scrollHeight
  }, [logs, paused])

  const filtered = logs.filter(l => {
    if (lvlFilter && l.l !== lvlFilter) return false
    if (search && !l.m.toLowerCase().includes(search.toLowerCase()) && !l.s.toLowerCase().includes(search.toLowerCase())) return false
    return true
  })

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100%' }}>
      <div style={{ display: 'flex', alignItems: 'center', gap: 8, padding: '10px 16px', borderBottom: `1px solid ${C.brd}`, flexShrink: 0, background: C.bg }}>
        <select value={lvlFilter} onChange={e => setLvlFilter(e.target.value)} style={{ background: C.bg3, border: `1px solid ${C.brd}`, color: C.txt, padding: '5px 8px', fontFamily: 'IBM Plex Mono, monospace', fontSize: 11, outline: 'none' }}>
          <option value="">All levels</option>
          <option value="INFO">INFO</option>
          <option value="WARN">WARN</option>
          <option value="ERR">ERR</option>
        </select>
        <input value={search} onChange={e => setSearch(e.target.value)} placeholder="Filter logs..." style={{ background: C.bg3, border: `1px solid ${C.brd}`, color: C.txt, padding: '5px 10px', fontFamily: 'IBM Plex Mono, monospace', fontSize: 11, width: 180, outline: 'none' }} />
        <div style={{ flex: 1 }} />
        <Btn small onClick={() => setPaused(v => !v)} style={{ borderColor: paused ? C.amb : C.brd, color: paused ? C.amb : C.dim }}>{paused ? 'Resume' : 'Pause'}</Btn>
        <Btn small onClick={() => setLogs([])}>Clear</Btn>
        <div style={{ display: 'flex', alignItems: 'center', gap: 5, fontSize: 10, color: paused ? C.amb : C.g }}>
          <span style={{ width: 5, height: 5, background: paused ? C.amb : C.g, borderRadius: '50%', display: 'inline-block', animation: paused ? 'none' : 'pulse 2s infinite' }} />
          {paused ? 'PAUSED' : 'STREAMING'}
        </div>
      </div>
      <div ref={bodyRef} style={{ flex: 1, overflowY: 'auto', padding: '8px 16px', fontFamily: 'IBM Plex Mono, monospace', fontSize: 11, lineHeight: 1.6 }}>
        {filtered.map(l => (
          <div key={l.id} style={{ display: 'flex', gap: 12, padding: '2px 0', borderBottom: `1px solid ${C.brd}30` }}>
            <span style={{ color: C.dim, flexShrink: 0, width: 60 }}>{l.ts}</span>
            <span style={{ color: LC[l.l] || C.dim, width: 40, flexShrink: 0, textAlign: 'right' }}>{l.l}</span>
            <span style={{ color: C.dim, width: 100, flexShrink: 0 }}>[{l.s}]</span>
            <span style={{ color: C.txt, flex: 1 }}>{l.m}</span>
          </div>
        ))}
      </div>
    </div>
  )
}
