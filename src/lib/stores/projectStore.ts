/**
 * 项目状态管理 Store — 管理项目生命周期、设备列表和多设备切换。
 *
 * 支持三种项目类型：
 * - `single_file`: 单 CSV 文件直接打开
 * - `aidps_folder`: AIDPS 文件夹扫描（多设备多 CSV 自动合并）
 * - `vibproj_file`: `.vibproj` 项目文件
 */
import { writable, derived, get } from 'svelte/store';
import { invoke } from '@tauri-apps/api/core';
import type { ColumnMapping, VibrationDataset } from '$lib/types/vibration';
import { addDeviceDataset } from './dataStore';

// ---------------------------------------------------------------------------
// 类型定义
// ---------------------------------------------------------------------------

/** 项目来源类型 */
export type ProjectType = 'single_file' | 'aidps_folder' | 'vibproj_file';

/** 数据源描述 — 一个文件对应一条记录 */
export interface DataSource {
  file_path: string;
  file_name: string;
  source_type: 'csv' | 'wav';
}

/** 通道分组 schema — groups 的 key 是组名（如 "vibration"），value 是通道名列表 */
export interface ChannelSchema {
  groups: Record<string, string[]>;
}

/** 设备信息 — 一个设备可包含多个数据源和通道 */
export interface DeviceInfo {
  id: string;
  name: string;
  sources: DataSource[];
  channel_schema: ChannelSchema;
}

/** 项目元数据 */
export interface ProjectMetadata {
  name: string;
  created_at: string;
  description?: string;
}

/** 前端项目状态完整描述 */
export interface ProjectState {
  project_type: ProjectType;
  devices: DeviceInfo[];
  /** 传感器位置到设备 ID 的映射（预留扩展） */
  sensor_mapping: Record<string, string>;
  metadata: ProjectMetadata;
}

// ---------------------------------------------------------------------------
// Stores
// ---------------------------------------------------------------------------

/** 项目是否已打开（控制 UI 显示状态） */
export const projectOpen = writable<boolean>(false);

/** 当前项目完整状态（无项目时为 null） */
export const project = writable<ProjectState | null>(null);

/** 当前活跃设备 ID — 用于在多设备项目中切换 */
export const activeDeviceId = writable<string | null>(null);

/** 派生：所有设备 ID 列表 */
export const deviceIds = derived(project, ($project) =>
  $project ? $project.devices.map(d => d.id) : []
);

/** 派生：当前活跃设备的完整信息 */
export const activeDevice = derived(
  [project, activeDeviceId],
  ([$project, $activeId]) => {
    if (!$project || !$activeId) return null;
    return $project.devices.find(d => d.id === $activeId) ?? null;
  }
);

/** 派生：项目中的设备总数 */
export const deviceCount = derived(project, ($project) =>
  $project ? $project.devices.length : 0
);

// ---------------------------------------------------------------------------
// Actions
// ---------------------------------------------------------------------------

/**
 * 初始化项目状态并自动选中第一个设备。
 * 由 dataStore.addFile 或 AIDPS 扫描流程调用。
 * @param state - 后端返回或前端构建的项目状态
 */
export function setProject(state: ProjectState): void {
  project.set(state);
  projectOpen.set(true);
  if (state.devices.length > 0) {
    activeDeviceId.set(state.devices[0].id);
  }
}

/**
 * 关闭当前项目 — 清空所有项目相关状态。
 * 注意：不负责清理 dataStore 中的数据集，由 dataStore.closeAll 统一处理。
 */
export function closeProject(): void {
  project.set(null);
  projectOpen.set(false);
  activeDeviceId.set(null);
}

/**
 * 切换活跃设备。
 * @param deviceId - 目标设备 ID
 */
export function selectDevice(deviceId: string): void {
  activeDeviceId.set(deviceId);
}

/**
 * 将后端 ProjectInfo（Rust serde PascalCase enum）转换为前端 ProjectState（snake_case）。
 * 例如 Rust 的 `ProjectType::SingleFile` 序列化为 `"SingleFile"`，需映射为 `"single_file"`。
 * @param info - 后端原始 JSON 对象
 * @returns 转换后的 ProjectState
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

/**
 * 加载 AIDPS 项目中某个设备的振动数据。
 * 从设备的 channel_schema 中提取通道列表，构建 ColumnMapping 后调用后端合并 CSV。
 * 加载成功后将数据集写入 dataStore。
 * @param deviceId - 要加载的设备 ID
 */
export async function loadDeviceData(deviceId: string): Promise<void> {
  const proj = get(project);
  if (!proj) return;

  const device = proj.devices.find(d => d.id === deviceId);
  if (!device) return;

  const filePaths = device.sources.map(s => s.file_path);

  // 将 channel_schema 中所有分组的通道名展平为 data_columns
  const allChannels = Object.values(device.channel_schema.groups).flat();
  const mapping: ColumnMapping = {
    time_column: 'time', // AIDPS 默认时间列名；后端会解析实际列
    data_columns: allChannels.length > 0 ? allChannels : [],
  };

  try {
    const ds = await invoke<VibrationDataset>('load_device_data', {
      deviceId,
      filePaths,
      mapping,
    });
    await addDeviceDataset(ds);
  } catch (e) {
    console.error('Failed to load device data:', e);
  }
}
