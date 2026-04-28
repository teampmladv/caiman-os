/**
 * framework/actions/registry.ts
 *
 * The Action Registry is the single source of truth for every operation
 * the Caimán UI can execute. Each action is a pure descriptor — no logic,
 * no side effects. The ActionBus (bus/index.ts) is responsible for
 * dispatching, confirming, executing and tracking them.
 *
 * Benefits over ad-hoc onClick handlers:
 *   - Keyboard shortcuts bound to action IDs, not components
 *   - ⌘K searches the registry for contextual suggestions
 *   - Every execution is automatically logged to the audit trail
 *   - Confirmation dialogs are declared, not scattered across components
 *   - Progress tracking for async ops is uniform
 *   - Permission checks run in one place
 */

import type { LucideIcon } from 'lucide-react'
import {
  Play, StopCircle, RefreshCw, Terminal, Cpu, HardDrive,
  ShieldCheck, Zap, PlusCircle, Trash2, AlertTriangle,
  Move, BarChart2, Settings, Download, Eye,
} from 'lucide-react'

// ── Action category ───────────────────────────────────────────────────────

export type ActionCategory =
  | 'vm.lifecycle'
  | 'vm.migration'
  | 'vm.console'
  | 'drs'
  | 'microseg'
  | 'storage'
  | 'gpu'
  | 'cluster'
  | 'monitoring'

// ── Confirmation level ────────────────────────────────────────────────────

export type ConfirmLevel =
  | 'none'       // execute immediately
  | 'toast'      // show undo toast (5s)
  | 'dialog'     // require typed confirmation
  | 'critical'   // require typing the resource name

// ── Action descriptor ─────────────────────────────────────────────────────

export interface ActionDef<TInput = void> {
  /** Unique identifier — used for shortcuts, audit log, ⌘K search */
  id:          string
  /** Human label (shown in buttons, ⌘K, audit log) */
  label:       string
  /** Short description for ⌘K suggestions and tooltips */
  description: string
  /** Icon component */
  icon:        LucideIcon
  /** Category for grouping in ⌘K and audit trail */
  category:    ActionCategory
  /** Keyboard shortcut (optional) */
  shortcut?:   string
  /** Confirmation requirements */
  confirm:     ConfirmLevel
  /** If dialog: message shown to user */
  confirmMsg?: (input: TInput) => string
  /** For critical: the string the user must type */
  confirmWord?:(input: TInput) => string
  /** Estimated duration for progress display */
  durationMs?: number
  /** Whether this action can be undone */
  undoable:    boolean
  /** Tags for ⌘K fuzzy search */
  tags:        string[]
  /** Permissions required */
  requires?:   string[]
}

// ── Typed action inputs ───────────────────────────────────────────────────

export interface VmRef    { vmId: string;  vmName: string }
export interface MigrateInput extends VmRef { toNode: string }
export interface CreateVmInput { memMib: number; cpus: number; kernel?: string }
export interface PolicyInput   { name: string; namespace: string; spec: unknown }
export interface VsanInput     { name: string; sizeGib: number; ftt: number }
export interface ExecuteDrsInput { vmId: string; fromNode: string; toNode: string }

// ── Registry ──────────────────────────────────────────────────────────────

export const ACTIONS = {

  // ── VM Lifecycle ─────────────────────────────────────────────────────────
  'vm.start': {
    id: 'vm.start', label: 'Start VM', icon: Play,
    description: 'Start a stopped VM on the cluster',
    category: 'vm.lifecycle', shortcut: 'mod+shift+s',
    confirm: 'none', undoable: false, durationMs: 15_000,
    tags: ['start', 'boot', 'run', 'power on'],
  } satisfies ActionDef<VmRef>,

  'vm.stop': {
    id: 'vm.stop', label: 'Stop VM', icon: StopCircle,
    description: 'Gracefully stop a running VM (SIGTERM to VMM)',
    category: 'vm.lifecycle', shortcut: 'mod+shift+x',
    confirm: 'toast',
    confirmMsg: (i: VmRef) => `Stop "${i.vmName}"? Running workloads will be suspended.`,
    undoable: true, durationMs: 5_000,
    tags: ['stop', 'shutdown', 'power off', 'halt'],
  } satisfies ActionDef<VmRef>,

  'vm.restart': {
    id: 'vm.restart', label: 'Restart VM', icon: RefreshCw,
    description: 'Graceful restart (stop + start)',
    category: 'vm.lifecycle',
    confirm: 'toast',
    confirmMsg: (i: VmRef) => `Restart "${i.vmName}"?`,
    undoable: false, durationMs: 20_000,
    tags: ['restart', 'reboot', 'reset'],
  } satisfies ActionDef<VmRef>,

  'vm.force-stop': {
    id: 'vm.force-stop', label: 'Force stop VM', icon: AlertTriangle,
    description: 'Forcibly kill VMM process — data loss possible',
    category: 'vm.lifecycle',
    confirm: 'dialog',
    confirmMsg: (i: VmRef) => `Force kill "${i.vmName}"? This may corrupt disk state.`,
    undoable: false,
    tags: ['kill', 'force stop', 'sigkill', 'terminate'],
    requires: ['admin'],
  } satisfies ActionDef<VmRef>,

  'vm.delete': {
    id: 'vm.delete', label: 'Delete VM', icon: Trash2,
    description: 'Permanently delete VM and its state files',
    category: 'vm.lifecycle',
    confirm: 'critical',
    confirmMsg: (i: VmRef) => `This will permanently delete "${i.vmName}" and all its data.`,
    confirmWord: (i: VmRef) => i.vmName,
    undoable: false,
    tags: ['delete', 'remove', 'destroy', 'purge'],
    requires: ['admin'],
  } satisfies ActionDef<VmRef>,

  'vm.create': {
    id: 'vm.create', label: 'Create VM', icon: PlusCircle,
    description: 'Create and start a new VM',
    category: 'vm.lifecycle', shortcut: 'mod+shift+n',
    confirm: 'none', undoable: false, durationMs: 30_000,
    tags: ['new', 'create', 'provision', 'launch'],
  } satisfies ActionDef<CreateVmInput>,

  // ── VM Migration ──────────────────────────────────────────────────────────
  'vm.migrate': {
    id: 'vm.migrate', label: 'Live migrate VM', icon: Move,
    description: 'Live migrate VM to another node with < 200ms downtime',
    category: 'vm.migration', shortcut: 'mod+m',
    confirm: 'toast',
    confirmMsg: (i: MigrateInput) =>
      `Migrate "${i.vmName}" to ${i.toNode}? Expect ~50–200ms blackout.`,
    undoable: false, durationMs: 120_000,
    tags: ['migrate', 'move', 'vmotion', 'live migration', 'rebalance'],
  } satisfies ActionDef<MigrateInput>,

  // ── VM Console ────────────────────────────────────────────────────────────
  'vm.console': {
    id: 'vm.console', label: 'Open serial console', icon: Terminal,
    description: 'Open VM serial console (ttyS0) in the side panel',
    category: 'vm.console', shortcut: 'mod+`',
    confirm: 'none', undoable: false,
    tags: ['console', 'serial', 'terminal', 'ttyS0', 'logs'],
  } satisfies ActionDef<VmRef>,

  'vm.logs': {
    id: 'vm.logs', label: 'View boot logs', icon: Eye,
    description: 'Tail the last 200 lines of serial console output',
    category: 'vm.console',
    confirm: 'none', undoable: false,
    tags: ['logs', 'boot', 'serial', 'output', 'kernel'],
  } satisfies ActionDef<VmRef>,

  // ── DRS ───────────────────────────────────────────────────────────────────
  'drs.execute-one': {
    id: 'drs.execute-one', label: 'Execute DRS migration', icon: Zap,
    description: 'Execute a specific DRS-recommended live migration',
    category: 'drs',
    confirm: 'toast',
    confirmMsg: (i: ExecuteDrsInput) =>
      `Migrate ${i.vmId} from ${i.fromNode} to ${i.toNode}?`,
    undoable: false, durationMs: 120_000,
    tags: ['drs', 'balance', 'migration', 'rebalance', 'execute'],
  } satisfies ActionDef<ExecuteDrsInput>,

  'drs.execute-all': {
    id: 'drs.execute-all', label: 'Execute all DRS recommendations', icon: Zap,
    description: 'Execute all pending DRS migration recommendations',
    category: 'drs',
    confirm: 'dialog',
    confirmMsg: () => 'Execute all pending DRS migrations? Multiple VMs will experience brief downtime.',
    undoable: false, durationMs: 300_000,
    tags: ['drs', 'execute all', 'balance cluster', 'rebalance all'],
    requires: ['admin'],
  } satisfies ActionDef<void>,

  'drs.set-mode': {
    id: 'drs.set-mode', label: 'Change DRS mode', icon: Settings,
    description: 'Switch DRS between Manual / SemiAutomated / FullyAutomated',
    category: 'drs',
    confirm: 'none', undoable: true,
    tags: ['drs', 'mode', 'manual', 'auto', 'semi-automated'],
  } satisfies ActionDef<{ mode: string }>,

  // ── Micro-segmentation ────────────────────────────────────────────────────
  'microseg.create-policy': {
    id: 'microseg.create-policy', label: 'Create policy', icon: ShieldCheck,
    description: 'Create a new MicroSegPolicy (XDP enforcement)',
    category: 'microseg', shortcut: 'mod+shift+p',
    confirm: 'none', undoable: true,
    tags: ['policy', 'microseg', 'allow', 'deny', 'firewall', 'xdp', 'zero trust'],
  } satisfies ActionDef<PolicyInput>,

  'microseg.delete-policy': {
    id: 'microseg.delete-policy', label: 'Delete policy', icon: Trash2,
    description: 'Delete a MicroSegPolicy — traffic will follow default-deny',
    category: 'microseg',
    confirm: 'dialog',
    confirmMsg: (i: { name: string }) =>
      `Delete policy "${i.name}"? Traffic covered by this rule will be DENIED.`,
    undoable: false,
    tags: ['delete', 'policy', 'microseg', 'remove rule'],
  } satisfies ActionDef<{ name: string }>,

  // ── Storage ───────────────────────────────────────────────────────────────
  'storage.create-volume': {
    id: 'storage.create-volume', label: 'Create VSAN volume', icon: HardDrive,
    description: 'Create a new distributed VSAN volume',
    category: 'storage',
    confirm: 'none', undoable: false, durationMs: 10_000,
    tags: ['vsan', 'volume', 'storage', 'disk', 'create', 'provision'],
  } satisfies ActionDef<VsanInput>,

  'storage.snapshot': {
    id: 'storage.snapshot', label: 'Take snapshot', icon: Download,
    description: 'Take a point-in-time snapshot of a VSAN volume',
    category: 'storage',
    confirm: 'none', undoable: false, durationMs: 5_000,
    tags: ['snapshot', 'backup', 'point in time', 'vsan'],
  } satisfies ActionDef<{ volumeId: string }>,

  // ── GPU ───────────────────────────────────────────────────────────────────
  'gpu.allocate-mig': {
    id: 'gpu.allocate-mig', label: 'Allocate MIG slice', icon: Cpu,
    description: 'Create a new NVIDIA MIG slice and attach to a VM',
    category: 'gpu',
    confirm: 'none', undoable: false, durationMs: 15_000,
    tags: ['mig', 'gpu', 'nvidia', 'slice', 'allocate', 'a100', 'h100'],
  } satisfies ActionDef<{ vmId: string; profile: string }>,

  'gpu.release': {
    id: 'gpu.release', label: 'Release GPU', icon: Cpu,
    description: 'Release GPU/MIG allocation from a VM',
    category: 'gpu',
    confirm: 'toast',
    confirmMsg: (i: VmRef) => `Release GPU from "${i.vmName}"?`,
    undoable: false,
    tags: ['gpu', 'release', 'detach', 'mig', 'vgpu'],
  } satisfies ActionDef<VmRef>,

  // ── Monitoring ────────────────────────────────────────────────────────────
  'monitoring.export-metrics': {
    id: 'monitoring.export-metrics', label: 'Export metrics CSV', icon: BarChart2,
    description: 'Export current cluster metrics as CSV',
    category: 'monitoring',
    confirm: 'none', undoable: false,
    tags: ['export', 'csv', 'metrics', 'prometheus', 'download'],
  } satisfies ActionDef<{ timeRange: string }>,

} as const

export type ActionId = keyof typeof ACTIONS

// ── Lookup helpers ────────────────────────────────────────────────────────

export function getAction(id: ActionId): ActionDef<unknown> {
  return ACTIONS[id] as ActionDef<unknown>
}

export function getActionsByCategory(cat: ActionCategory): ActionDef<unknown>[] {
  return Object.values(ACTIONS).filter(a => a.category === cat) as ActionDef<unknown>[]
}

export function searchActions(query: string): ActionDef<unknown>[] {
  const q = query.toLowerCase()
  return (Object.values(ACTIONS) as ActionDef<unknown>[]).filter(a =>
    a.label.toLowerCase().includes(q)
    || a.description.toLowerCase().includes(q)
    || a.tags.some(t => t.includes(q))
  )
}
