import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export type TyperState = {
  enabled: boolean;
  volume: number;
  pack: string;
  pack_name: string;
};

export type PackInfo = {
  id: string;
  name: string;
  builtin: boolean;
};

/** False when the UI runs in a plain browser (e.g. `vite dev`) for previewing. */
export const inTauri = isTauri();

const tauriBackend = {
  getState: () => invoke<TyperState>("get_state"),
  setEnabled: (enabled: boolean) =>
    invoke<TyperState>("set_enabled", { enabled }),
  setVolume: (volume: number) => invoke<TyperState>("set_volume", { volume }),
  setPack: (id: string) => invoke<TyperState>("set_pack", { id }),
  preview: () => invoke<TyperState>("preview"),
  listPacks: () => invoke<PackInfo[]>("list_packs"),
  openPacksDir: () => invoke<void>("open_packs_dir"),
  close: () => invoke<void>("close"),
  /** Enable close-on-blur once the user has reached the popup. */
  armAutoClose: () => invoke<void>("arm_autoclose"),
  /** `null` means the daemon went away. */
  onState: (cb: (state: TyperState | null) => void) =>
    listen<TyperState | null>("state-changed", (e) => cb(e.payload)),
};

/** In-memory stand-in so the popup can be previewed in a browser. */
function mockBackend(): typeof tauriBackend {
  const packs: PackInfo[] = [
    { id: "builtin:click", name: "Click (built-in)", builtin: true },
    { id: "builtin:thock", name: "Thock (built-in)", builtin: true },
    { id: "cherrymx-blue", name: "CherryMX Blue", builtin: false },
    { id: "topre", name: "Topre", builtin: false },
  ];
  let state: TyperState = {
    enabled: true,
    volume: 60,
    pack: packs[0].id,
    pack_name: packs[0].name,
  };
  const update = (patch: Partial<TyperState>) => {
    state = { ...state, ...patch };
    return Promise.resolve(state);
  };
  return {
    getState: () => Promise.resolve(state),
    setEnabled: (enabled) => update({ enabled }),
    setVolume: (volume) => update({ volume }),
    setPack: (id) => {
      const pack = packs.find((p) => p.id === id);
      return pack
        ? update({ pack: pack.id, pack_name: pack.name })
        : Promise.reject(new Error(`unknown pack ${id}`));
    },
    preview: () => Promise.resolve(state),
    listPacks: () => Promise.resolve(packs),
    openPacksDir: () => Promise.resolve(),
    close: () => Promise.resolve(),
    armAutoClose: () => Promise.resolve(),
    onState: () => Promise.resolve(() => {}),
  };
}

export const typer = inTauri ? tauriBackend : mockBackend();
