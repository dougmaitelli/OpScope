import "./DialogCloseButton.css";

export function DialogCloseButton({ label, onClick }: { label: string; onClick: () => void }) {
  return (
    <button className="dialog-close-button" type="button" aria-label={label} onClick={onClick} />
  );
}
