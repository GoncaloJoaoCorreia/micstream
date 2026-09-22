import { useState, useEffect } from "react";
import {
  Sliders,
  X,
  Volume2,
  Clock,
  Radio,
  RotateCcw,
  Check,
  AlertTriangle,
} from "lucide-react";
import type { AppSettings, TransportMode } from "../types";

export interface SettingsModalProps {
  isOpen: boolean;
  onClose: () => void;
  settings: AppSettings;
  onSave: (settings: AppSettings) => void;
  disabled?: boolean;
}

export const DEFAULT_APP_SETTINGS: AppSettings = {
  mode: "opus",
  target_jitter_ms: 5.0,
  udp_port: 48124,
};

export function SettingsModal({
  isOpen,
  onClose,
  settings,
  onSave,
  disabled = false,
}: SettingsModalProps) {
  const [draftMode, setDraftMode] = useState<TransportMode>(settings.mode);
  const [draftJitter, setDraftJitter] = useState<number>(settings.target_jitter_ms);
  const [draftPort, setDraftPort] = useState<number>(settings.udp_port);

  // Sync draft when modal opens or settings prop changes
  useEffect(() => {
    if (isOpen) {
      setDraftMode(settings.mode);
      setDraftJitter(settings.target_jitter_ms);
      setDraftPort(settings.udp_port);
    }
  }, [isOpen, settings]);

  // Handle Escape key
  useEffect(() => {
    if (!isOpen) return;
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        onClose();
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [isOpen, onClose]);

  if (!isOpen) return null;

  const handleResetDefaults = () => {
    setDraftMode(DEFAULT_APP_SETTINGS.mode);
    setDraftJitter(DEFAULT_APP_SETTINGS.target_jitter_ms);
    setDraftPort(DEFAULT_APP_SETTINGS.udp_port);
  };

  const handleSave = () => {
    const validPort =
      draftPort >= 1024 && draftPort <= 65535
        ? draftPort
        : DEFAULT_APP_SETTINGS.udp_port;
    const validJitter = Math.min(20, Math.max(2.5, draftJitter));

    onSave({
      mode: draftMode,
      target_jitter_ms: validJitter,
      udp_port: validPort,
    });
    onClose();
  };

  const sampleCount = Math.round(draftJitter * 48);

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-slate-950/80 backdrop-blur-sm animate-in fade-in duration-150">
      <div
        className="relative w-full max-w-lg bg-slate-900 border border-slate-800 rounded-2xl shadow-2xl overflow-hidden flex flex-col"
        role="dialog"
        aria-modal="true"
        aria-labelledby="settings-modal-title"
      >
        {/* Header */}
        <div className="flex items-center justify-between px-6 py-4 border-b border-slate-800">
          <div className="flex items-center gap-2.5">
            <div className="p-2 rounded-lg bg-emerald-500/10 text-emerald-400 border border-emerald-500/20">
              <Sliders className="w-4 h-4" />
            </div>
            <div>
              <h2
                id="settings-modal-title"
                className="text-base font-semibold text-slate-100"
              >
                Audio & Network Settings
              </h2>
              <p className="text-xs text-slate-400">
                Configure codec latency, jitter buffering, and transport ports
              </p>
            </div>
          </div>

          <button
            type="button"
            onClick={onClose}
            className="p-1.5 rounded-lg text-slate-400 hover:text-slate-200 hover:bg-slate-800 transition cursor-pointer"
            aria-label="Close settings"
          >
            <X className="w-5 h-5" />
          </button>
        </div>

        {/* Content Body */}
        <div className="p-6 flex flex-col gap-5 overflow-y-auto max-h-[75vh]">
          {disabled && (
            <div className="flex items-center gap-2 p-3 rounded-lg bg-amber-500/10 border border-amber-500/20 text-amber-300 text-xs">
              <AlertTriangle className="w-4 h-4 shrink-0 text-amber-400" />
              <span>
                A stream is currently active. Saved settings will apply on your next stream session.
              </span>
            </div>
          )}

          {/* 1. Codec Selector */}
          <div className="flex flex-col gap-2.5">
            <div className="flex items-center gap-2 text-xs font-semibold text-slate-300">
              <Volume2 className="w-4 h-4 text-emerald-400" />
              <span>Audio Codec & Transport Mode</span>
            </div>

            <div className="grid grid-cols-1 gap-2.5">
              {/* Opus Mode Card */}
              <button
                type="button"
                onClick={() => setDraftMode("opus")}
                className={`flex flex-col p-3 rounded-xl border text-left transition cursor-pointer ${
                  draftMode === "opus"
                    ? "bg-emerald-950/30 border-emerald-500/50 shadow-sm shadow-emerald-950"
                    : "bg-slate-950/60 border-slate-800/80 hover:border-slate-700"
                }`}
              >
                <div className="flex items-center justify-between">
                  <div className="flex items-center gap-2">
                    <span className="text-sm font-semibold text-slate-100">
                      Opus LowDelay
                    </span>
                    <span className="text-[10px] font-mono px-1.5 py-0.5 rounded bg-emerald-500/20 text-emerald-300 border border-emerald-500/30 font-medium">
                      Recommended
                    </span>
                  </div>
                  {draftMode === "opus" && (
                    <Check className="w-4 h-4 text-emerald-400" />
                  )}
                </div>
                <p className="text-xs text-slate-400 mt-1">
                  5ms frames (240 samples @ 48kHz, 96 kbps CBR). Low bandwidth with built-in Packet Loss Concealment (PLC) for stable Wi-Fi.
                </p>
              </button>

              {/* Raw PCM Mode Card */}
              <button
                type="button"
                onClick={() => setDraftMode("raw_pcm")}
                className={`flex flex-col p-3 rounded-xl border text-left transition cursor-pointer ${
                  draftMode === "raw_pcm"
                    ? "bg-emerald-950/30 border-emerald-500/50 shadow-sm shadow-emerald-950"
                    : "bg-slate-950/60 border-slate-800/80 hover:border-slate-700"
                }`}
              >
                <div className="flex items-center justify-between">
                  <div className="flex items-center gap-2">
                    <span className="text-sm font-semibold text-slate-100">
                      Raw PCM (Uncompressed)
                    </span>
                    <span className="text-[10px] font-mono px-1.5 py-0.5 rounded bg-blue-500/20 text-blue-300 border border-blue-500/30 font-medium">
                      &lt;16ms Latency
                    </span>
                  </div>
                  {draftMode === "raw_pcm" && (
                    <Check className="w-4 h-4 text-emerald-400" />
                  )}
                </div>
                <p className="text-xs text-slate-400 mt-1">
                  16-bit signed LE @ 48kHz mono (768 kbps). Zero compression delay for ultra-low latency on wired Ethernet LAN.
                </p>
              </button>
            </div>
          </div>

          {/* 2. Adaptive Jitter Buffer Target Watermark */}
          <div className="flex flex-col gap-2.5 pt-2 border-t border-slate-800/80">
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-2 text-xs font-semibold text-slate-300">
                <Clock className="w-4 h-4 text-emerald-400" />
                <span>Host Jitter Buffer Watermark</span>
              </div>
              <span className="text-xs font-mono font-bold text-emerald-400">
                {draftJitter.toFixed(1)} ms{" "}
                <span className="text-slate-500 font-normal">
                  ({sampleCount} smp)
                </span>
              </span>
            </div>

            <p className="text-xs text-slate-400">
              Target buffer depth on the Host receiver before playback. Absorbs network packet jitter while maintaining dynamic micro-resampling synchronization.
            </p>

            <div className="flex flex-col gap-2 pt-1">
              <input
                type="range"
                min="2.5"
                max="20.0"
                step="0.5"
                value={draftJitter}
                onChange={(e) => setDraftJitter(parseFloat(e.target.value))}
                className="w-full accent-emerald-500 bg-slate-800 rounded-lg h-2 cursor-pointer"
              />
              <div className="flex justify-between text-[10px] text-slate-500 font-mono">
                <span>2.5 ms (Ultra-Low)</span>
                <span>5.0 ms (Default)</span>
                <span>10.0 ms (Wi-Fi)</span>
                <span>20.0 ms (High Loss)</span>
              </div>
            </div>

            {/* Quick preset chips */}
            <div className="flex gap-2 mt-1">
              {[
                { label: "Ultra-Low (2.5ms)", val: 2.5 },
                { label: "Balanced (5.0ms)", val: 5.0 },
                { label: "Wi-Fi Safe (10.0ms)", val: 10.0 },
              ].map((preset) => (
                <button
                  key={preset.val}
                  type="button"
                  onClick={() => setDraftJitter(preset.val)}
                  className={`text-[11px] px-2.5 py-1 rounded-md border transition cursor-pointer ${
                    Math.abs(draftJitter - preset.val) < 0.1
                      ? "bg-emerald-950/60 border-emerald-500/40 text-emerald-300 font-medium"
                      : "bg-slate-950/60 border-slate-800 text-slate-400 hover:text-slate-200"
                  }`}
                >
                  {preset.label}
                </button>
              ))}
            </div>
          </div>

          {/* 3. UDP Port */}
          <div className="flex flex-col gap-2 pt-2 border-t border-slate-800/80">
            <div className="flex items-center gap-2 text-xs font-semibold text-slate-300">
              <Radio className="w-4 h-4 text-emerald-400" />
              <span>Default UDP Port</span>
            </div>

            <div className="flex items-center gap-3">
              <input
                type="number"
                min={1024}
                max={65535}
                value={draftPort}
                onChange={(e) => setDraftPort(Number(e.target.value))}
                className="w-32 px-3 py-2 rounded-lg bg-slate-950/80 border border-slate-800 text-xs text-slate-200 font-mono focus:outline-none focus:ring-1 focus:ring-emerald-500"
              />
              <span className="text-xs text-slate-400">
                Default: <span className="font-mono text-slate-300">48124</span>. Ensure firewall permits UDP on both client and host.
              </span>
            </div>
          </div>
        </div>

        {/* Footer */}
        <div className="flex items-center justify-between px-6 py-4 border-t border-slate-800 bg-slate-900/50">
          <button
            type="button"
            onClick={handleResetDefaults}
            className="flex items-center gap-1.5 text-xs text-slate-400 hover:text-slate-200 transition cursor-pointer"
          >
            <RotateCcw className="w-3.5 h-3.5" />
            <span>Reset to Defaults</span>
          </button>

          <div className="flex items-center gap-2.5">
            <button
              type="button"
              onClick={onClose}
              className="px-4 py-2 rounded-lg border border-slate-800 text-xs font-medium text-slate-300 hover:bg-slate-800/70 transition cursor-pointer"
            >
              Cancel
            </button>
            <button
              type="button"
              onClick={handleSave}
              className="px-4 py-2 rounded-lg bg-emerald-600 hover:bg-emerald-500 text-xs font-medium text-white transition shadow-sm shadow-emerald-950 cursor-pointer"
            >
              Save Settings
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
