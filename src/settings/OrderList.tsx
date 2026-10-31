type Props = {
  ids: readonly string[];
  label: (id: string) => string;
  onMove: (id: string, direction: -1 | 1) => void;
  onReset?: () => void;
  resetLabel: string;
};

export function OrderList({ ids, label, onMove, onReset, resetLabel }: Props) {
  return (
    <div className="panel-order-row">
      {ids.map((id, index) => (
        <div key={id} className="panel-order-item">
          <button className="panel-order-arrow" disabled={index === 0} onClick={() => onMove(id, -1)}>‹</button>
          <span>{label(id)}</span>
          <button className="panel-order-arrow" disabled={index === ids.length - 1} onClick={() => onMove(id, 1)}>›</button>
        </div>
      ))}
      {onReset && <button className="panel-order-reset" onClick={onReset}>{resetLabel}</button>}
    </div>
  );
}
