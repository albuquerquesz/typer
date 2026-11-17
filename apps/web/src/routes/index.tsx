import { createFileRoute } from "@tanstack/react-router";
import { cn } from "@typer/ui/lib/utils";
import {
  Check,
  FolderOpen,
  Keyboard,
  KeyRound,
  Move,
  Play,
  Power,
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

    window.addEventListener("pointermove", typer.armAutoClose, {
      once: true,
    });
    return () => {
      unlisten.then((fn) => fn());
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
        <Switch
          checked={state.enabled}
          label="Sons do teclado"
          title="Super + Shift + T"
          onChange={(enabled) => run(typer.setEnabled(enabled))}
        />
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

      <Item
        role="menuitemcheckbox"
        aria-checked={state.overlay}
        onClick={() => run(typer.setOverlay(!state.overlay))}
      >
        <KeyRound className="size-3.5 shrink-0" />
        <span className="flex-1">Mostrar teclas</span>
        <Switch checked={state.overlay} label="Mostrar teclas" />
      </Item>
      {state.overlay && (
        <Item onClick={() => run(typer.moveOverlay())}>
          <Move className="size-3.5 shrink-0" /> Mover…
        </Item>
      )}

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

      <Separator />

      <Item
        title="Para o typerd: sons, ícone na barra e teclas na tela"
        className="hover:text-destructive"
        onClick={() => run(typer.quitDaemon())}
      >
        <Power className="size-3.5 shrink-0" /> Encerrar typer
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

/** On/off pill. Without `onChange` it is display-only (its row handles clicks). */
function Switch({
  checked,
  label,
  title,
  onChange,
}: {
  checked: boolean;
  label: string;
  title?: string;
  onChange?: (checked: boolean) => void;
}) {
  const className = cn(
    "relative h-4 w-7 shrink-0 rounded-full transition-colors",
    checked ? "bg-primary" : "bg-muted-foreground/40",
  );
  const knob = (
    <span
      className={cn(
        "bg-background absolute top-0.5 left-0.5 size-3 rounded-full transition-transform",
        checked && "translate-x-3",
      )}
    />
  );
  if (!onChange) {
    return (
      <span aria-hidden className={className}>
        {knob}
      </span>
    );
  }
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      title={title}
      onClick={() => onChange(!checked)}
      className={className}
    >
      {knob}
    </button>
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
  // hidden until the content is laid out and the window fits it; then the
  // window is mapped (`ready`) and the open animation starts.
  const [phase, setPhase] = useState<"hidden" | "open" | "closing">("hidden");
  const ready = children !== undefined;

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

  useEffect(() => {
    const el = ref.current;
    if (!ready || !el) return;
    let frame = 0;
    typer
      .fit(el.getBoundingClientRect().height)
      .then(typer.ready)
      .finally(() => {
        // Two frames: let GTK apply the new size before the first animated one.
        frame = requestAnimationFrame(() => {
          frame = requestAnimationFrame(() => setPhase("open"));
        });
      });
    return () => cancelAnimationFrame(frame);
  }, [ready]);

  // Esc, clicking elsewhere and a second tray click all close with an animation.
  useEffect(() => {
    let timer: ReturnType<typeof setTimeout>;
    const dismiss = () => {
      setPhase("closing");
      // Fallback for reduced motion, where no animationend fires.
      timer = setTimeout(typer.close, 150);
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") dismiss();
    };
    window.addEventListener("keydown", onKey);
    const unlisten = typer.onDismiss(dismiss);
    return () => {
      clearTimeout(timer);
      window.removeEventListener("keydown", onKey);
      unlisten.then((fn) => fn());
    };
  }, []);

  const menu = (
    <main
      ref={ref}
      data-phase={phase}
      onAnimationEnd={() => {
        if (phase === "closing") typer.close();
      }}
      className={cn(
        "menu border-border bg-background text-foreground w-[260px] rounded-lg border p-1 text-xs select-none",
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
