import { Gauge, ShieldCheck, Cpu } from "lucide-react";
import type { StreamTelemetry, TransportMode, AppRole } from "../types";

export interface ConnectionStatsProps {
  telemetry: StreamTelemetry;
  mode: TransportMode;
  isActive: boolean;
  role: AppRole;
}

export function ConnectionStats({
  telemetry,
  mode,
  isActive,
  role,
}: ConnectionStatsProps) {
  const rtt = telemetry.rtt_ms;
  const loss = telemetry.packet_loss_percent;
  const jitter = telemetry.jitter_ms;

  // Determine quality tier
  const getRttColor = (val: number) => {
    if (!isActive) return "text-slate-400";
    if (val <= 0.0) return "text-emerald-400";
    if (val < 25) return "text-emerald-400";
    if (val < 50) return "text-amber-400";
    return "text-rose-400";
  };

  const getLossColor = (val: number) => {
    if (!isActive) return "text-slate-400";
    if (val === 0) return "text-emerald-400";
    if (val < 2.0) return "text-amber-400";
    return "text-rose-400";
  };

  const getJitterColor = (val: number) => {
    if (!isActive) return "text-slate-400";
    if (val < 5.0) return "text-emerald-400";
    if (val < 15.0) return "text-amber-400";
    return "text-rose-400";
  };

  const codecLabel =
    mode === "opus"
      ? "Opus LowDelay (5ms / 96k)"
      : "Raw PCM (16-bit 48kHz)";

  const networkQuality = !isActive
    ? "Standby"
    : loss > 3.0 || rtt > 60
    ? "Degraded"
    : loss > 0.5 || rtt > 30
    ? "Good"
    : "Optimal";

  const networkQualityBadgeColor = !isActive
    ? "bg-slate-800 text-slate-400 border-slate-700"
    : networkQuality === "Optimal"
    ? "bg-emerald-500/10 text-emerald-400 border-emerald-500/20"
    : networkQuality === "Good"
    ? "bg-amber-500/10 text-amber-400 border-amber-500/20"
    : "bg-rose-500/10 text-rose-400 border-rose-500/20";

  return (
    <div className="p-4 rounded-xl bg-slate-900/70 border border-slate-800 flex flex-col gap-3 shadow-sm">
      {/* Header with Title and Quality Badge */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <div className="p-1 rounded bg-emerald-500/10 text-emerald-400">
            <Gauge className="w-4 h-4" />
          </div>
          <span className="text-xs font-semibold text-slate-200">
            Connection Telemetry
          </span>
          <span className="text-[10px] text-slate-500">
            {role === "client" ? "(Client Loopback)" : "(Host Receiver)"}
          </span>
        </div>

        <div className="flex items-center gap-2">
          <span
            className={`text-[10px] font-medium px-2 py-0.5 rounded-full border ${networkQualityBadgeColor}`}
          >
            {networkQuality}
          </span>
        </div>
      </div>

      {/* Metric Badges Grid */}
      <div className="grid grid-cols-4 gap-2">
        {/* Metric 1: RTT Latency */}
        <div className="flex flex-col p-2.5 rounded-lg bg-slate-950/70 border border-slate-800/80">
          <span className="text-[10px] uppercase tracking-wider text-slate-500 font-medium">
            RTT Latency
          </span>
          <div className="flex items-baseline gap-1 mt-1">
            <span className={`text-base font-bold font-mono ${getRttColor(rtt)}`}>
              {isActive ? (rtt > 0 ? rtt.toFixed(1) : "< 1.0") : "--"}
            </span>
            <span className="text-[10px] text-slate-500 font-mono">ms</span>
          </div>
        </div>

        {/* Metric 2: Packet Loss */}
        <div className="flex flex-col p-2.5 rounded-lg bg-slate-950/70 border border-slate-800/80">
          <span className="text-[10px] uppercase tracking-wider text-slate-500 font-medium">
            Packet Loss
          </span>
          <div className="flex items-baseline gap-1 mt-1">
            <span className={`text-base font-bold font-mono ${getLossColor(loss)}`}>
              {isActive ? loss.toFixed(1) : "--"}
            </span>
            <span className="text-[10px] text-slate-500 font-mono">%</span>
          </div>
        </div>

        {/* Metric 3: Jitter */}
        <div className="flex flex-col p-2.5 rounded-lg bg-slate-950/70 border border-slate-800/80">
          <span className="text-[10px] uppercase tracking-wider text-slate-500 font-medium">
            Jitter
          </span>
          <div className="flex items-baseline gap-1 mt-1">
            <span className={`text-base font-bold font-mono ${getJitterColor(jitter)}`}>
              {isActive ? jitter.toFixed(1) : "--"}
            </span>
            <span className="text-[10px] text-slate-500 font-mono">ms</span>
          </div>
        </div>

        {/* Metric 4: Packets Delivered */}
        <div className="flex flex-col p-2.5 rounded-lg bg-slate-950/70 border border-slate-800/80">
          <span className="text-[10px] uppercase tracking-wider text-slate-500 font-medium">
            Packets
          </span>
          <div className="flex items-baseline gap-1 mt-1">
            <span className="text-base font-bold font-mono text-slate-300">
              {isActive
                ? telemetry.packets_sent > 1000
                  ? `${(telemetry.packets_sent / 1000).toFixed(1)}k`
                  : telemetry.packets_sent
                : "--"}
            </span>
            {isActive && telemetry.packets_lost > 0 && (
              <span className="text-[9px] text-rose-400 font-mono">
                (-{telemetry.packets_lost})
              </span>
            )}
          </div>
        </div>
      </div>

      {/* Codec & Pipeline Footer */}
      <div className="flex items-center justify-between text-[11px] text-slate-400 px-1 pt-1 border-t border-slate-800/60">
        <div className="flex items-center gap-1.5">
          <Cpu className="w-3.5 h-3.5 text-slate-500" />
          <span>Active Codec:</span>
          <span className="font-mono text-slate-300 font-medium">{codecLabel}</span>
        </div>

        <div className="flex items-center gap-1.5 text-slate-500">
          <ShieldCheck className="w-3.5 h-3.5 text-emerald-500/70" />
          <span>Framing: 20B Header</span>
        </div>
      </div>
    </div>
  );
}
