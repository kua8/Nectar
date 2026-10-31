import { DndContext, PointerSensor, closestCenter, useSensor, useSensors, type DragEndEvent } from "@dnd-kit/core";
import { SortableContext, arrayMove, rectSortingStrategy, useSortable } from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";

type Props = {
  ids: readonly string[];
  label: (id: string) => string;
  onReorder: (ids: string[]) => void;
  onReset?: () => void;
  resetLabel: string;
};

function Chip({ id, label, index, total, onMove }: { id: string; label: string; index: number; total: number; onMove: (from: number, to: number) => void }) {
  const { attributes, listeners, setNodeRef, transform, transition, isDragging } = useSortable({ id });
  const style = {
    transform: CSS.Transform.toString(transform),
    transition,
    opacity: isDragging ? 0.5 : 1,
    zIndex: isDragging ? 5 : undefined,
    cursor: "grab",
  };
  return (
    <div ref={setNodeRef} style={style} className="panel-order-item" {...attributes} {...listeners}>
      <button className="panel-order-arrow" disabled={index === 0} onPointerDown={(e) => e.stopPropagation()} onClick={() => onMove(index, index - 1)}>‹</button>
      <span>{label}</span>
      <button className="panel-order-arrow" disabled={index === total - 1} onPointerDown={(e) => e.stopPropagation()} onClick={() => onMove(index, index + 1)}>›</button>
    </div>
  );
}

export function OrderList({ ids, label, onReorder, onReset, resetLabel }: Props) {
  const sensors = useSensors(useSensor(PointerSensor, { activationConstraint: { distance: 5 } }));
  const list = [...ids];

  const move = (from: number, to: number) => onReorder(arrayMove(list, from, to));

  const onDragEnd = (event: DragEndEvent) => {
    const { active, over } = event;
    if (!over || active.id === over.id) return;
    move(list.indexOf(String(active.id)), list.indexOf(String(over.id)));
  };

  return (
    <div className="panel-order-row">
      <DndContext sensors={sensors} collisionDetection={closestCenter} onDragEnd={onDragEnd}>
        <SortableContext items={list} strategy={rectSortingStrategy}>
          {list.map((id, index) => (
            <Chip key={id} id={id} label={label(id)} index={index} total={list.length} onMove={move} />
          ))}
        </SortableContext>
      </DndContext>
      {onReset && <button className="panel-order-reset" onClick={onReset}>{resetLabel}</button>}
    </div>
  );
}
