import "./DialogCloseButton.css";

export function DialogCloseButton({
  label,
  onClick,
  disabled = false,
}: {
  label: string;
  onClick: () => void;
  disabled?: boolean;
}) {
  return (
    <button
      className="dialog-close-button"
      type="button"
      aria-label={label}
      onClick={onClick}
      disabled={disabled}
    />
  );
}
