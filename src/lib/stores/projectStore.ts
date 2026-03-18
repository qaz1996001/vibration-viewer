import { writable, derived, get } from 'svelte/store';
import { invoke } from '@tauri-apps/api/core';
import type { ColumnMapping, VibrationDataset } from '$lib/types/vibration';
import { addDeviceDataset } from './dataStore';

// --- Types ---

export type ProjectType = 'single_file' | 'aidps_folder' | 'vibproj_file';

export interface DataSource {
  file_path: string;
  file_name: string;
  source_type: 'csv' | 'wav';
}

export interface ChannelSchema {
  groups: Record<string, string[]>;
}

export interface DeviceInfo {
  id: string;
  name: string;
  sources: DataSource[];
  channel_schema: ChannelSchema;
}

export interface ProjectMetadata {
  name: string;
  created_at: string;
  description?: string;
}

export interface ProjectState {
  project_type: ProjectType;
  devices: DeviceInfo[];
  sensor_mapping: Record<string, string>;
  metadata: ProjectMetadata;
}

// --- Stores ---

/** Whether a project is currently open */
export const projectOpen = writable<boolean>(false);

/** Current project info (null when no project open) */
export const project = writable<ProjectState | null>(null);

/** Currently active device ID */
export const activeDeviceId = writable<string | null>(null);

/** Derived: list of device IDs */
export const deviceIds = derived(project, ($project) =>
  $project ? $project.devices.map(d => d.id) : []
);

/** Derived: active device info */
export const activeDevice = derived(
  [project, activeDeviceId],
  ([$project, $activeId]) => {
    if (!$project || !$activeId) return null;
    return $project.devices.find(d => d.id === $activeId) ?? null;
  }
);

/** Derived: number of devices */
export const deviceCount = derived(project, ($project) =>
  $project ? $project.devices.length : 0
);

// --- Actions ---

/** Initialize project from backend ProjectInfo */
export function setProject(state: ProjectState): void {
  project.set(state);
  projectOpen.set(true);
  // Auto-select first device
  if (state.devices.length > 0) {
    activeDeviceId.set(state.devices[0].id);
  }
}

/** Close current project */
export function closeProject(): void {
  project.set(null);
  projectOpen.set(false);
  activeDeviceId.set(null);
}

/** Switch active device */
export function selectDevice(deviceId: string): void {
  activeDeviceId.set(deviceId);
}

/**
 * Convert backend ProjectInfo (PascalCase enum) to frontend ProjectState (snake_case).
 * Rust serde serializes ProjectType as "SingleFile", "AidpsFolder", "VibprojFile".
 */
export function mapBackendProjectInfo(info: Record<string, unknown>): ProjectState {
  const typeMap: Record<string, ProjectType> = {
    'SingleFile': 'single_file',
    'AidpsFolder': 'aidps_folder',
    'VibprojFile': 'vibproj_file',
  };
  const rawType = info.project_type as string;
  return {
    project_type: typeMap[rawType] ?? 'single_file',
    devices: (info.devices as DeviceInfo[]) ?? [],
    sensor_mapping: (info.sensor_mapping as Record<string, string>) ?? {},
    metadata: info.metadata as ProjectMetadata,
  };
}

/** Load device data from AIDPS project (triggers backend multi-CSV merge) */
export async function loadDeviceData(deviceId: string): Promise<void> {
  const proj = get(project);
  if (!proj) return;

  const device = proj.devices.find(d => d.id === deviceId);
  if (!device) return;

  // Extract file paths from device sources
  const filePaths = device.sources.map(s => s.file_path);

  // Build ColumnMapping from device channel_schema
  // Flatten all group channels into data_columns; use first source's time column assumption
  const allChannels = Object.values(device.channel_schema.groups).flat();
  const mapping: ColumnMapping = {
    time_column: 'time', // AIDPS default; backend will resolve actual column
    data_columns: allChannels.length > 0 ? allChannels : [],
  };

  try {
    const ds = await invoke<VibrationDataset>('load_device_data', {
      deviceId,
      filePaths,
      mapping,
    });
    // Populate dataStore with the loaded dataset
    await addDeviceDataset(ds);
  } catch (e) {
    console.error('Failed to load device data:', e);
  }
}
