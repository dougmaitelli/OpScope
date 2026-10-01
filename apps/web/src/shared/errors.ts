export function requestErrorMessage(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }
  if (typeof error === "object" && error !== null && "message" in error) {
    const message = (error as { message: unknown }).message;
    if (typeof message === "string") {
      return message;
    }
  }
  return "The request could not be completed. Try again.";
}

export function requestErrorSentence(error: unknown): string {
  const message = requestErrorMessage(error).trim();
  if (!message) return "The request could not be completed.";
  const sentence = message.charAt(0).toLocaleUpperCase() + message.slice(1);
  return /[.!?…]$/.test(sentence) ? sentence : `${sentence}.`;
}
