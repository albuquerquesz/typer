import { createFileRoute } from "@tanstack/react-router";
import { Button } from "@typer/ui/components/button";
import { cn } from "@typer/ui/lib/utils";
import {
  FolderOpen,
  Keyboard,
  Play,
  Power,
  Volume1,
  Volume2,
  VolumeX,
} from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { toast } from "sonner";
import { inTauri, type PackInfo, type TyperState, typer } from "@/lib/typer";

export const Route = createFileRoute("/")({
  component: Popup,
});

const errorMessage = (e: unknown) =>
  e instanceof Error ? e.message : String(e);

function Popup() {
  const [state, setState] = useState<TyperState | null>(null);
  const [packs, setPacks] = useState<PackInfo[]>([]);
  const [down, setDown] = useState(false);
  const [volume, setVolume] = useState(0);
  const dragging = useRef(false);
  const volumeTimer = useRef<ReturnType<typeof setTimeout>>(undefined);

  useEffect(() => {
    const apply = (s: TyperState | null) => {
      setDown(s === null);
      if (!s) return;
      setState(s);
      if (!dragging.current) setVolume(s.volume);
    };
    typer.getState().then(apply, () => apply(null));
    typer.listPacks().then(setPacks, () => {});
    const unlisten = typer.onState(apply);

    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") typer.close();
    };
    window.addEventListener("keydown", onKey);
    window.addEventListener("pointermove", typer.armAutoClose, {
      once: true,
    });
    return () => {
      unlisten.then((fn) => fn());
      window.removeEventListener("keydown", onKey);
      window.removeEventListener("pointermove", typer.armAutoClose);
    };
  }, []);

  const run = (action: Promise<TyperState>) =>
    action.then(setState, (e) => toast.error(errorMessage(e)));

  const changeVolume = (value: number) => {
    setVolume(value);
    clearTimeout(volumeTimer.current);
    volumeTimer.current = setTimeout(() => run(typer.setVolume(value)), 50);
  };

  const changePack = (id: string) =>
    run(typer.setPack(id).then(() => typer.preview()));

  if (down) {
    return (
      <Shell>
        <div className="flex flex-1 flex-col items-center justify-center gap-2 text-center">
          <Keyboard className="text-muted-foreground size-6" />
          <p className="text-sm font-medium">typerd não está rodando</p>
          <code className="bg-muted px-2 py-1 text-xs">
            systemctl --user start typerd
          </code>
        </div>
      </Shell>
    );
  }

  if (!state) return <Shell />;

  const VolumeIcon = volume === 0 ? VolumeX : volume < 50 ? Volume1 : Volume2;

  return (
    <Shell>
      <section className="flex items-center justify-between">
        <div>
          <p className="text-sm font-medium">
            {state.enabled ? "Sons ligados" : "Sons desligados"}
          </p>
          <p className="text-muted-foreground text-xs">Super + Shift + T</p>
        </div>
        <Button
          size="icon"
          variant={state.enabled ? "default" : "outline"}
          aria-label={state.enabled ? "Desligar" : "Ligar"}
          aria-pressed={state.enabled}
          onClick={() => run(typer.setEnabled(!state.enabled))}
        >
          <Power />
        </Button>
      </section>

      <section
        className={cn("space-y-1.5", !state.enabled && "opacity-50")}
        aria-label="Volume"
      >
        <div className="flex items-center justify-between text-xs">
          <span className="text-muted-foreground flex items-center gap-1.5">
            <VolumeIcon className="size-3.5" /> Volume
          </span>
          <span className="tabular-nums">{volume}%</span>
        </div>
        <input
          type="range"
          min={0}
          max={100}
          value={volume}
          aria-label="Volume"
          className="accent-primary w-full"
          onPointerDown={() => {
            dragging.current = true;
          }}
          onPointerUp={() => {
            dragging.current = false;
          }}
          onChange={(e) => changeVolume(Number(e.currentTarget.value))}
        />
      </section>

      <section className="flex min-h-0 flex-1 flex-col gap-1.5">
        <div className="flex items-center justify-between">
          <span className="text-muted-foreground text-xs">Som</span>
          <div className="flex gap-0.5">
            <Button
              size="icon-xs"
              variant="ghost"
              aria-label="Testar som"
              onClick={() => run(typer.preview())}
            >
              <Play />
            </Button>
            <Button
              size="icon-xs"
              variant="ghost"
              aria-label="Abrir pasta de packs"
              title="Coloque packs do Mechvibes aqui"
              onClick={() =>
                typer.openPacksDir().catch((e) => toast.error(errorMessage(e)))
              }
            >
              <FolderOpen />
            </Button>
          </div>
        </div>
        <ul className="border-border min-h-0 flex-1 overflow-y-auto border">
          {packs.map((p) => (
            <li key={p.id}>
              <button
                type="button"
                onClick={() => changePack(p.id)}
                className={cn(
                  "hover:bg-muted flex w-full items-center justify-between px-2 py-1.5 text-left text-xs",
                  p.id === state.pack && "bg-muted font-medium",
                )}
              >
                <span className="truncate">{p.name}</span>
                {p.id === state.pack && <span aria-hidden>●</span>}
              </button>
            </li>
          ))}
        </ul>
      </section>
    </Shell>
  );
}

function Shell({ children }: { children?: React.ReactNode }) {
  const popup = (
    <main
      className={cn(
        "border-border bg-background text-foreground flex flex-col gap-4 overflow-hidden rounded-xl border p-3 select-none",
        inTauri ? "h-svh" : "h-[300px] w-[380px] shadow-2xl",
      )}
    >
      <header className="-mb-1 flex items-center gap-1.5 text-sm font-semibold">
        <Keyboard className="size-4" /> typer
      </header>
      {children}
    </main>
  );
  if (inTauri) return popup;
  return (
    <div className="bg-muted flex min-h-svh items-start justify-center p-4 pt-12">
      {popup}
    </div>
  );
}
