interface Props {
  count: number;
}

export function ObservationFooter({ count }: Props) {
  if (count === 0) return null;
  return (
    <p className="observation-footer">
      You have resolved this pattern {count} time{count === 1 ? "" : "s"} before.
    </p>
  );
}
