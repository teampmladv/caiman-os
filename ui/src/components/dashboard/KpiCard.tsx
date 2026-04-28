import React from 'react'
import { clsx } from 'clsx'

type Variant = 'ok' | 'warn' | 'info' | 'crit'

interface KpiCardProps {
  label:    string
  value:    string
  unit:     string
  sub:      string
  variant:  Variant
  delta:    string
  deltaUp:  boolean
}

const accent: Record<Variant, string> = {
  ok:   'bg-caiman-bright',
  warn: 'bg-caiman-amber',
  info: 'bg-caiman-blue',
  crit: 'bg-caiman-red',
}

export function KpiCard({ label, value, unit, sub, variant, delta, deltaUp }: KpiCardProps) {
  return (
    <div className="kpi-card p-3 relative">
      <div className={`absolute bottom-0 left-0 right-0 h-[2px] ${accent[variant]}`} />
      <div className="text-[8px] text-caiman-dim tracking-[2px] uppercase mb-1">{label}</div>
      <div className="text-[24px] font-light text-[#e8f5e9] leading-none tabular-nums font-mono">
        {value}
        {unit && <span className="text-[11px] text-caiman-green ml-1">{unit}</span>}
      </div>
      <div className="text-[9px] text-caiman-dim mt-1">{sub}</div>
      <div className={clsx(
        'absolute top-2.5 right-2.5 text-[8px] px-1.5 py-0.5 rounded',
        deltaUp ? 'bg-[#0d2e0d] text-caiman-bright' : 'bg-[#1a0d0d] text-caiman-red',
      )}>
        {delta}
      </div>
    </div>
  )
}
