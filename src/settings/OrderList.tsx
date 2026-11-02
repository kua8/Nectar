import type { ReactNode } from "react";
import { DndContext, KeyboardSensor, PointerSensor, closestCenter, useSensor, useSensors, type DragEndEvent } from "@dnd-kit/core";
import { SortableContext, arrayMove, sortableKeyboardCoordinates, useSortable, verticalListSortingStrategy } from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import { GripVertical, RotateCcw } from "lucide-react";

type Props = {
  ids: readonly string[];
  label: (id: string) => string;
  icon?: (id: string) => ReactNode;
  onReorder: (ids: string[]) => void;
  onReset?: () => void;
  resetLabel: string;
};

function Row({ id, label, icon, position }: { id: string; label: string; icon?: ReactNode; position: number }) {
  const { attributes, listeners, setNodeRef, setActivatorNodeRef, transform, transition, isDragging } = useSortable({ id });
  const style = {
    transform: CSS.Transform.toString(transform),
    transition,
    zIndex: isDragging ? 5 : undefined,
  };
  return (
    <div ref={setNodeRef} style={style} className={`panel-order-item${isDragging ? " dragging" : ""}`}>
      <button ref={setActivatorNodeRef} type="button" className="panel-order-grip" aria-label={label} {...attributes} {...listeners}>
        <GripVertical size={14} strokeWidth={2} />
      </button>
      {icon && <span className="panel-order-icon">{icon}</span>}
      <span className="panel-order-name">{label}</span>
      <span className="panel-order-position">{position}</span>
    </div>
  );
}

export function OrderList({ ids, label, icon, onReorder, onReset, resetLabel }: Props) {
  const sensors = useSensors(
    useSensor(PointerSensor, { activationConstraint: { distance: 4 } }),
    useSensor(KeyboardSensor, { coordinateGetter: sortableKeyboardCoordinates })
  );
  const list = [...ids];

  const onDragEnd = (event: DragEndEvent) => {
    const { active, over } = event;
    if (!over || active.id === over.id) return;
    onReorder(arrayMove(list, list.indexOf(String(active.id)), list.indexOf(String(over.id))));
  };

  return (
    <div className="panel-order-list">
      <DndContext sensors={sensors} collisionDetection={closestCenter} onDragEnd={onDragEnd}>
        <SortableContext items={list} strategy={verticalListSortingStrategy}>
          {list.map((id, index) => (
            <Row key={id} id={id} label={label(id)} icon={icon?.(id)} position={index + 1} />
          ))}
        </SortableContext>
      </DndContext>
      {onReset && (
        <button type="button" className="panel-order-reset" onClick={onReset}>
          <RotateCcw size={12} />
          {resetLabel}
        </button>
      )}
    </div>
  );
}
