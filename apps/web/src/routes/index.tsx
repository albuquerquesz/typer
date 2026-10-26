import { createFileRoute } from "@tanstack/react-router";
import { cn } from "@typer/ui/lib/utils";
import {
  Check,
  FolderOpen,
  Keyboard,
  Play,
  Volume1,
  Volume2,
  VolumeX,
} from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { inTauri, type PackInfo, type TyperState, typer } from "@/lib/typer";

export const Route = createFileRoute("/")({
  component: Menu,
});

const errorMessage = (e: unknown) =>
  e instanceof Error ? e.message : String(e);

function Menu() {
  const [state, setState] = useState<TyperState | null>(null);
  const [packs, setPacks] = useState<PackInfo[]>([]);
  const [down, setDown] = useState(false);
  const [error, setError] = useState<string | null>(null);
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

  const run = (action: Promise<unknown>) =>
    action.then(
      (s) => {
        setError(null);
        if (s) setState(s as TyperState);
      },
      (e) => setError(errorMessage(e)),
    );

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
        <p className="text-muted-foreground px-2 py-1.5">
          typerd não está rodando
        </p>
        <code className="bg-muted mx-1 mb-1 block rounded px-2 py-1 text-[11px]">
          systemctl --user start typerd
        </code>
      </Shell>
    );
  }

  if (!state) return <Shell />;

  const VolumeIcon = volume === 0 ? VolumeX : volume < 50 ? Volume1 : Volume2;

  return (
    <Shell
      toggle={
        <button
          type="button"
          role="switch"
          aria-checked={state.enabled}
          aria-label="Sons do teclado"
          title="Super + Shift + T"
          onClick={() => run(typer.setEnabled(!state.enabled))}
          className={cn(
            "relative h-4 w-7 shrink-0 rounded-full transition-colors",
            state.enabled ? "bg-primary" : "bg-muted-foreground/40",
          )}
        >
          <span
            className={cn(
              "bg-background absolute top-0.5 left-0.5 size-3 rounded-full transition-transform",
              state.enabled && "translate-x-3",
            )}
          />
        </button>
      }
    >
      <label
        className={cn(
          "flex h-7 items-center gap-2 px-2",
          !state.enabled && "opacity-50",
        )}
      >
        <VolumeIcon className="text-muted-foreground size-3.5 shrink-0" />
        <input
          type="range"
          min={0}
          max={100}
          value={volume}
          aria-label="Volume"
          className="accent-primary h-1 min-w-0 flex-1"
          onPointerDown={() => {
            dragging.current = true;
          }}
          onPointerUp={() => {
            dragging.current = false;
          }}
          onChange={(e) => changeVolume(Number(e.currentTarget.value))}
        />
        <span className="text-muted-foreground w-8 text-right tabular-nums">
          {volume}%
        </span>
      </label>

      <Separator />

      <ul className="max-h-[168px] overflow-y-auto" aria-label="Som">
        {packs.map((p) => (
          <li key={p.id}>
            <Item
              onClick={() => changePack(p.id)}
              aria-current={p.id === state.pack}
            >
              <Check
                className={cn(
                  "size-3.5 shrink-0",
                  p.id !== state.pack && "invisible",
                )}
              />
              <span className="truncate">{p.name}</span>
            </Item>
          </li>
        ))}
      </ul>

      <Separator />

      <Item onClick={() => run(typer.preview())}>
        <Play className="size-3.5 shrink-0" /> Testar som
      </Item>
      <Item
        title="Coloque packs do Mechvibes aqui"
        onClick={() => run(typer.openPacksDir())}
      >
        <FolderOpen className="size-3.5 shrink-0" /> Abrir pasta de packs
      </Item>

      {error && (
        <p className="text-destructive px-2 py-1 text-[11px] break-words">
          {error}
        </p>
      )}
    </Shell>
  );
}

function Item({ className, ...props }: React.ComponentProps<"button">) {
  return (
    <button
      type="button"
      className={cn(
        "hover:bg-muted flex h-7 w-full items-center gap-2 rounded-md px-2 text-left",
        className,
      )}
      {...props}
    />
  );
}

function Separator() {
  return <hr className="border-border -mx-1 my-1" />;
}

function Shell({
  toggle,
  children,
}: {
  toggle?: React.ReactNode;
  children?: React.ReactNode;
}) {
  const ref = useRef<HTMLElement>(null);

  // The window has no fixed height: it follows the menu.
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const observer = new ResizeObserver(() => {
      typer.fit(el.getBoundingClientRect().height);
    });
    observer.observe(el);
    return () => observer.disconnect();
  }, []);

  const menu = (
    <main
      ref={ref}
      className={cn(
        "border-border bg-background text-foreground w-[260px] rounded-lg border p-1 text-xs select-none",
        !inTauri && "shadow-xl",
      )}
    >
      <header className="flex h-7 items-center gap-2 px-2 font-semibold">
        <Keyboard className="size-3.5" />
        <span className="flex-1">typer</span>
        {toggle}
      </header>
      {children}
    </main>
  );
  if (inTauri) return menu;
  return (
    <div className="bg-muted flex min-h-svh items-start justify-center p-4 pt-12">
      {menu}
    </div>
  );
}
