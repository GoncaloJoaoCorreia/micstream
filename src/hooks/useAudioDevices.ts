import { useState, useEffect, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { AudioDeviceInfo, VirtualDriverStatus } from "../types";

export interface UseAudioDevicesResult {
  inputDevices: AudioDeviceInfo[];
  outputDevices: AudioDeviceInfo[];
  selectedInput: string | undefined;
  selectedOutput: string | undefined;
  virtualDriverStatus: VirtualDriverStatus | null;
  isLoading: boolean;
  error: string | null;
  refreshDevices: () => Promise<void>;
  setSelectedInput: (deviceName: string) => void;
  setSelectedOutput: (deviceName: string) => void;
}

export function useAudioDevices(): UseAudioDevicesResult {
  const [inputDevices, setInputDevices] = useState<AudioDeviceInfo[]>([]);
  const [outputDevices, setOutputDevices] = useState<AudioDeviceInfo[]>([]);
  const [selectedInput, setSelectedInputState] = useState<string | undefined>(undefined);
  const [selectedOutput, setSelectedOutputState] = useState<string | undefined>(undefined);
  const [virtualDriverStatus, setVirtualDriverStatus] = useState<VirtualDriverStatus | null>(null);
  const [isLoading, setIsLoading] = useState<boolean>(false);
  const [error, setError] = useState<string | null>(null);

  const isMountedRef = useRef<boolean>(true);
  const selectedInputRef = useRef<string | undefined>(selectedInput);
  const selectedOutputRef = useRef<string | undefined>(selectedOutput);

  selectedInputRef.current = selectedInput;
  selectedOutputRef.current = selectedOutput;

  const refreshDevices = useCallback(async () => {
    setIsLoading(true);
    setError(null);

    try {
      const [inputs, outputs, driverStatus] = await Promise.all([
        invoke<AudioDeviceInfo[]>("get_input_devices"),
        invoke<AudioDeviceInfo[]>("get_output_devices"),
        invoke<VirtualDriverStatus>("get_virtual_driver_status"),
      ]);

      if (!isMountedRef.current) return;

      setInputDevices(inputs);
      setOutputDevices(outputs);
      setVirtualDriverStatus(driverStatus);

      // Handle default / current selection for input
      const currentInput = selectedInputRef.current;
      const inputStillExists = inputs.some((d) => d.name === currentInput);
      if (!currentInput || !inputStillExists) {
        const defaultInput = inputs.find((d) => d.is_default) ?? inputs[0];
        if (defaultInput) {
          setSelectedInputState(defaultInput.name);
        }
      }

      // Handle default / current selection for output (prefer virtual driver on host)
      const currentOutput = selectedOutputRef.current;
      const outputStillExists = outputs.some((d) => d.name === currentOutput);
      if (!currentOutput || !outputStillExists) {
        const virtualOutput = outputs.find((d) => d.is_virtual);
        const defaultOutput = outputs.find((d) => d.is_default);
        const preferred = virtualOutput ?? defaultOutput ?? outputs[0];
        if (preferred) {
          setSelectedOutputState(preferred.name);
        }
      }
    } catch (err) {
      if (isMountedRef.current) {
        const message = err instanceof Error ? err.message : String(err);
        setError(message);
      }
    } finally {
      if (isMountedRef.current) {
        setIsLoading(false);
      }
    }
  }, []);

  const setSelectedInput = useCallback((deviceName: string) => {
    setSelectedInputState(deviceName);
  }, []);

  const setSelectedOutput = useCallback((deviceName: string) => {
    setSelectedOutputState(deviceName);
  }, []);

  // Fetch on mount and subscribe to window focus
  useEffect(() => {
    isMountedRef.current = true;
    refreshDevices();

    const handleWindowFocus = () => {
      refreshDevices();
    };

    window.addEventListener("focus", handleWindowFocus);

    return () => {
      isMountedRef.current = false;
      window.removeEventListener("focus", handleWindowFocus);
    };
  }, [refreshDevices]);

  return {
    inputDevices,
    outputDevices,
    selectedInput,
    selectedOutput,
    virtualDriverStatus,
    isLoading,
    error,
    refreshDevices,
    setSelectedInput,
    setSelectedOutput,
  };
}
