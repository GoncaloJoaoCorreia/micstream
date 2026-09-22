import React from "react";
import { Mic, Volume2, RotateCw, Sparkles, CheckCircle2 } from "lucide-react";
import type { AudioDeviceInfo } from "../types";

export interface DevicePickerProps {
  label: string;
  type: "input" | "output";
  devices: AudioDeviceInfo[];
  selectedDevice?: string;
  onSelectDevice: (deviceName: string) => void;
  onRefresh?: () => void;
  isLoading?: boolean;
  disabled?: boolean;
  placeholder?: string;
}

export const DevicePicker: React.FC<DevicePickerProps> = ({
  label,
  type,
  devices,
  selectedDevice,
  onSelectDevice,
  onRefresh,
  isLoading = false,
  disabled = false,
  placeholder = "Select an audio device...",
}) => {
  const currentDevice = devices.find((d) => d.name === selectedDevice);
  const Icon = type === "input" ? Mic : Volume2;

  const handleChange = (e: React.ChangeEvent<HTMLSelectElement>) => {
    onSelectDevice(e.target.value);
  };

  return (
    <div className="flex flex-col gap-2 p-4 rounded-xl bg-slate-900/70 border border-slate-800 backdrop-blur-sm transition">
      {/* Header with Title and Refresh Button */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <div className="p-1.5 rounded-lg bg-emerald-500/10 text-emerald-400 border border-emerald-500/20">
            <Icon className="w-4 h-4" />
          </div>
          <span className="text-sm font-medium text-slate-200">{label}</span>
        </div>

        {onRefresh && (
          <button
            type="button"
            onClick={onRefresh}
            disabled={isLoading || disabled}
            className="p-1.5 rounded-lg text-slate-400 hover:text-slate-200 hover:bg-slate-800 disabled:opacity-40 disabled:cursor-not-allowed transition"
            title="Refresh device list"
            aria-label="Refresh device list"
          >
            <RotateCw
              className={`w-3.5 h-3.5 ${isLoading ? "animate-spin text-emerald-400" : ""}`}
            />
          </button>
        )}
      </div>

      {/* Device Selector */}
      <div className="relative mt-1">
        <select
          value={selectedDevice ?? ""}
          onChange={handleChange}
          disabled={disabled || devices.length === 0}
          aria-label={label}
          className="w-full appearance-none py-2.5 pl-3.5 pr-10 rounded-lg bg-slate-950/70 border border-slate-800 text-sm text-slate-200 focus:outline-none focus:ring-1 focus:ring-emerald-500 focus:border-emerald-500 disabled:opacity-50 disabled:cursor-not-allowed transition cursor-pointer"
        >
          {devices.length === 0 ? (
            <option value="" disabled>
              No {type} devices found
            </option>
          ) : (
            <>
              {!selectedDevice && (
                <option value="" disabled>
                  {placeholder}
                </option>
              )}
              {devices.map((device) => {
                const tags: string[] = [];
                if (device.is_virtual) tags.push("Virtual Cable");
                if (device.is_default) tags.push("Default");
                const tagStr = tags.length > 0 ? ` (${tags.join(", ")})` : "";
                return (
                  <option key={device.id} value={device.name} className="bg-slate-900 text-slate-200">
                    {device.name}
                    {tagStr}
                  </option>
                );
              })}
            </>
          )}
        </select>

        {/* Custom Chevron Indicator */}
        <div className="pointer-events-none absolute inset-y-0 right-0 flex items-center pr-3 text-slate-400">
          <svg
            className="w-4 h-4"
            fill="none"
            stroke="currentColor"
            viewBox="0 0 24 24"
            xmlns="http://www.w3.org/2000/svg"
          >
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              strokeWidth="2"
              d="M19 9l-7 7-7-7"
            />
          </svg>
        </div>
      </div>

      {/* Selected Device Badges & Diagnostics */}
      {currentDevice && (
        <div className="flex flex-wrap items-center gap-2 mt-1">
          {currentDevice.is_virtual ? (
            <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-xs font-medium bg-emerald-500/10 text-emerald-400 border border-emerald-500/20">
              <Sparkles className="w-3 h-3" />
              Virtual Audio Cable
            </span>
          ) : (
            type === "output" && (
              <span className="text-xs text-amber-400/80">
                Tip: Use a virtual cable (BlackHole/VB-Cable) to route into Discord/OBS.
              </span>
            )
          )}

          {currentDevice.is_default && (
            <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-xs font-medium bg-slate-800 text-slate-300 border border-slate-700">
              <CheckCircle2 className="w-3 h-3 text-slate-400" />
              System Default
            </span>
          )}
        </div>
      )}
    </div>
  );
};
