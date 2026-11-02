import { useState } from "react";
import {
  DndContext,
  DragOverlay,
  PointerSensor,
  rectIntersection,
  useDraggable,
  useDroppable,
  useSensor,
  useSensors,
  type DragEndEvent,
  type DragStartEvent,
} from "@dnd-kit/core";
import { SortableContext, arrayMove, horizontalListSortingStrategy, useSortable } from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import { Bell, BatteryCharging, Bluetooth, ChevronUp, Moon, PanelBottom, PanelTop, RotateCcw, RotateCw, Settings, Sun, Volume2, Wifi, X } from "lucide-react";
import type { ComponentType } from "react";
import { resolveCcLayout, type CcKind, type CcLayout } from "../panelOrder";
import { useT } from "../i18n";

type Icon = ComponentType<{ size?: number; strokeWidth?: number }>;

const ITEMS: Record<string, { kind: CcKind; icon: Icon }> = {
  network: { kind: "tiles", icon: Wifi },
  "dock-mode": { kind: "tiles", icon: PanelBottom },
  bluetooth: { kind: "tiles", icon: Bluetooth },
  "notch-mode": { kind: "tiles", icon: PanelTop },
  dnd: { kind: "buttons", icon: Moon },
  "battery-saver": { kind: "buttons", icon: BatteryCharging },
  tray: { kind: "buttons", icon: ChevronUp },
  notifications: { kind: "buttons", icon: Bell },
  settings: { kind: "buttons", icon: Settings },
  restart: { kind: "buttons", icon: RotateCw },
  volume: { kind: "sliders", icon: Volume2 },
  brightness: { kind: "sliders", icon: Sun },
};

const KINDS: CcKind[] = ["tiles", "buttons", "sliders"];
const ZONE_LABEL: Record<CcKind, string> = { tiles: "notch.ccTiles", buttons: "notch.ccButtons", sliders: "notch.ccSliders" };

function PoolChip({ id, onAdd }: { id: string; onAdd: (id: string) => void }) {
  const t = useT();
  const { attributes, listeners, setNodeRef, isDragging } = useDraggable({ id });
  const Icon = ITEMS[id].icon;
  return (
    <div
      ref={setNodeRef}
      className={`widget-pill widget-pill--available ${isDragging ? "dragging" : ""}`}
      onClick={() => onAdd(id)}
      {...listeners}
      {...attributes}
    >
      <Icon size={12} strokeWidth={2} />
      <span>{t(`cc.layout.${id}`)}</span>
    </div>
  );
}

function PlacedChip({ id, onRemove }: { id: string; onRemove: (id: string) => void }) {
  const t = useT();
  const { attributes, listeners, setNodeRef, transform, transition, isDragging } = useSortable({ id });
  const Icon = ITEMS[id].icon;
  const style = { transform: CSS.Transform.toString(transform), transition, opacity: isDragging ? 0.4 : 1, zIndex: isDragging ? 50 : ("auto" as const) };
  return (
    <div ref={setNodeRef} style={style} className={`widget-pill widget-pill--placed ${isDragging ? "dragging" : ""}`}>
      <div {...attributes} {...listeners} className="widget-pill-drag-handle">
        <Icon size={12} strokeWidth={2} />
        <span>{t(`cc.layout.${id}`)}</span>
      </div>
      <div className="widget-pill-btns">
        <button className="widget-pill-btn widget-pill-btn--x" title={t("widgets.remove")} onClick={() => onRemove(id)}>
          <X size={10} />
        </button>
      </div>
    </div>
  );
}

function Zone({ kind, items, onRemove }: { kind: CcKind; items: string[]; onRemove: (id: string) => void }) {
  const t = useT();
  const { setNodeRef, isOver } = useDroppable({ id: `zone-${kind}` });
  return (
    <div ref={setNodeRef} id={`zone-${kind}`} className={`widget-config-zone ${isOver ? "widget-config-zone--over" : ""}`}>
      <span className="widget-config-side">{t(ZONE_LABEL[kind])}</span>
      <div className="widget-config-chips">
        <SortableContext items={items} strategy={horizontalListSortingStrategy}>
          {items.length > 0
            ? items.map((id) => <PlacedChip key={id} id={id} onRemove={onRemove} />)
            : <span className="widget-config-empty">{t("widgets.drop")}</span>}
        </SortableContext>
      </div>
    </div>
  );
}

type Props = {
  value: CcLayout;
  onChange: (layout: CcLayout) => void;
  onReset: () => void;
  customized: boolean;
};

export function ControlCenterConfig({ value, onChange, onReset, customized }: Props) {
  const t = useT();
  const layout = resolveCcLayout(value);
  const [activeId, setActiveId] = useState<string | null>(null);
  const sensors = useSensors(useSensor(PointerSensor, { activationConstraint: { distance: 5 } }));

  const placed = new Set(KINDS.flatMap((k) => layout[k]));
  const pool = Object.keys(ITEMS).filter((id) => !placed.has(id));

  const emit = (kind: CcKind, list: string[]) => onChange({ ...value, [kind]: list });
  const add = (id: string) => emit(ITEMS[id].kind, [...layout[ITEMS[id].kind], id]);
  const remove = (id: string) => emit(ITEMS[id].kind, layout[ITEMS[id].kind].filter((x) => x !== id));

  const onDragStart = (event: DragStartEvent) => setActiveId(String(event.active.id));

  const onDragEnd = (event: DragEndEvent) => {
    setActiveId(null);
    const { active, over } = event;
    if (!over) return;
    const id = String(active.id);
    const overId = String(over.id);
    const item = ITEMS[id];
    if (!item) return;
    const targetKind = overId.startsWith("zone-") ? (overId.slice(5) as CcKind) : ITEMS[overId]?.kind;
    if (targetKind !== item.kind) return;
    const list = layout[item.kind];
    if (pool.includes(id)) {
      const at = list.indexOf(overId);
      const next = [...list];
      if (at >= 0) next.splice(at, 0, id);
      else next.push(id);
      emit(item.kind, next);
      return;
    }
    const from = list.indexOf(id);
    const to = list.indexOf(overId);
    if (from >= 0 && to >= 0 && from !== to) emit(item.kind, arrayMove(list, from, to));
  };

  const activeItem = activeId ? ITEMS[activeId] : null;

  return (
    <DndContext sensors={sensors} collisionDetection={rectIntersection} onDragStart={onDragStart} onDragEnd={onDragEnd}>
      <div className="widget-config widget-config--control-center">
        {KINDS.map((kind) => (
          <Zone key={kind} kind={kind} items={layout[kind]} onRemove={remove} />
        ))}
        {pool.length > 0 && (
          <div className="widget-config-pool">
            <div className="widget-config-pool-chips">
              {pool.map((id) => <PoolChip key={id} id={id} onAdd={add} />)}
            </div>
          </div>
        )}
        {customized && <button className="panel-order-reset" onClick={onReset}><RotateCcw size={12} />{t("notch.panelOrder.reset")}</button>}
      </div>
      <DragOverlay>
        {activeItem && activeId ? (
          <div className="widget-pill widget-pill--dragging">
            <activeItem.icon size={12} strokeWidth={2} />
            <span>{t(`cc.layout.${activeId}`)}</span>
          </div>
        ) : null}
      </DragOverlay>
    </DndContext>
  );
}

