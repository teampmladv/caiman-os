import { useState, useRef, useEffect } from 'react'
import { useApp } from '../store.jsx'
import { C, Btn } from '../components/UI.jsx'

const BOOT_LOG = [
  ['g','[    0.000000] Linux version 6.6.69-0-virt'],
  ['d','[    0.000000] Command line: earlycon=uart8250,io,0x3f8,115200n8 console=ttyS0'],
  ['d','[    0.000000] BIOS-e820: [mem 0x0-0x9fbff] usable'],
  ['g','[    0.000000] printk: bootconsole [uart8250] enabled'],
  ['d','[    0.123922] printk: console [ttyS0] enabled'],
  ['g','[    0.237144] serial8250: ttyS0 at I/O 0x3f8 (irq=4) is a 16450'],
  ['g','[    0.897438] Run /init as init process'],
  ['g','[    0.903247] Alpine Init 3.9.1-r0'],
  ['g',''],
  ['w','caiman-vm login: root'],
  ['w','Password: '],
  ['g','Welcome to Caiman OS'],
  ['w',''],
  ['g','root@nginx-prod:~# '],
]

const RESPONSES = {
  uptime: '02:31:44 up 3 days, 12:04, 1 user, load average: 0.12, 0.08, 0.06',
  ls: 'bin  dev  etc  home  lib  proc  root  sys  tmp  usr  var',
  uname: 'Linux nginx-prod 6.6.69-0-virt #1-Alpine SMP x86_64',
  ps: '  PID TTY          TIME CMD\n    1 ttyS0    00:00:00 init\n   42 ttyS0    00:00:01 nginx\n  100 ttyS0    00:00:00 sh',
  free: '              total        used        free\nMem:         524288      184320      339968\nSwap:             0           0           0',
  df: 'Filesystem      Size  Used Avail Use% Mounted on\n/dev/vda        20G   3.2G   16G  17% /',
  'cat /proc/cpuinfo': 'processor\t: 0\nmodel name\t: AMD Ryzen 5 3600 6-Core\ncpu MHz\t\t: 3600.000',
}

export default function Console() {
  const { vms } = useApp()
  const [activeVM, setActiveVM] = useState(vms.find(v => v.status === 'RUNNING')?.id || '')
  const [lines, setLines] = useState(BOOT_LOG)
  const [input, setInput] = useState('')
  const bodyRef = useRef()

  const vm = vms.find(v => v.id === activeVM)

  useEffect(() => {
    if (bodyRef.current) bodyRef.current.scrollTop = bodyRef.current.scrollHeight
  }, [lines])

  const submit = (e) => {
    if (e.key !== 'Enter') return
    const cmd = input.trim()
    if (!cmd) return
    const out = RESPONSES[cmd] || `bash: ${cmd}: command not found`
    setLines(prev => [...prev, ['w', `root@${vm?.name || 'vm'}:~# ${cmd}`], ['d', out], ['g', `root@${vm?.name || 'vm'}:~# `]])
    setInput('')
  }

  const colorMap = { g: C.g, d: C.dim, b: C.blu, w: C.txt, r: C.red, a: C.amb }

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100%', background: '#020408' }}>
      {/* VM tabs */}
      <div style={{ display: 'flex', gap: 1, padding: '8px 12px', borderBottom: `1px solid ${C.brd}`, background: '#070b14', flexShrink: 0, overflowX: 'auto' }}>
        {vms.filter(v => v.status === 'RUNNING').map(v => (
          <button key={v.id} onClick={() => { setActiveVM(v.id); setLines(BOOT_LOG) }}
            style={{ padding: '4px 12px', background: activeVM === v.id ? `${C.g}15` : 'transparent', border: `1px solid ${activeVM === v.id ? C.g : C.brd}`, color: activeVM === v.id ? C.g : C.dim, fontFamily: 'IBM Plex Mono, monospace', fontSize: 10, cursor: 'pointer', whiteSpace: 'nowrap' }}>
            {v.name}
          </button>
        ))}
      </div>
      {/* Header */}
      <div style={{ display: 'flex', alignItems: 'center', gap: 8, padding: '8px 16px', background: '#0d1117', borderBottom: `1px solid ${C.brd}`, flexShrink: 0 }}>
        {['#ef5350','#ffb300',C.g].map((c, i) => <span key={i} style={{ width: 10, height: 10, borderRadius: '50%', background: c, display: 'inline-block' }} />)}
        <span style={{ fontSize: 11, color: C.dim, marginLeft: 4 }}>{vm?.name || 'no vm'} — ttyS0</span>
        <span style={{ marginLeft: 'auto', fontSize: 10, color: C.g, display: 'flex', alignItems: 'center', gap: 4 }}>
          <span style={{ width: 5, height: 5, background: C.g, borderRadius: '50%', display: 'inline-block', animation: 'pulse 2s infinite' }} />CONNECTED
        </span>
        <span style={{ fontSize: 10, color: C.dim }}>Serial console · KVM direct</span>
      </div>
      {/* Body */}
      <div ref={bodyRef} style={{ flex: 1, overflowY: 'auto', padding: 14, fontFamily: 'IBM Plex Mono, monospace', fontSize: 12, lineHeight: 1.6 }}>
        {lines.map((line, i) => (
          <div key={i} style={{ color: colorMap[line[0]] || C.txt, whiteSpace: 'pre-wrap' }}>{line[1] || '\u00a0'}</div>
        ))}
      </div>
      {/* Input */}
      <div style={{ display: 'flex', alignItems: 'center', gap: 8, padding: '8px 14px', background: '#0d1117', borderTop: `1px solid ${C.brd}`, flexShrink: 0 }}>
        <span style={{ color: C.g, fontSize: 12 }}>root@{vm?.name || 'vm'}:~#</span>
        <input value={input} onChange={e => setInput(e.target.value)} onKeyDown={submit}
          style={{ background: 'transparent', border: 'none', color: C.txt, fontFamily: 'IBM Plex Mono, monospace', fontSize: 12, flex: 1, outline: 'none' }}
          placeholder="type command..." autoFocus />
      </div>
    </div>
  )
}
