import { writable, derived } from 'svelte/store';

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
