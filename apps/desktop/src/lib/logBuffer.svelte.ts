export class LogBuffer {
  text = $state("");

  clear(): void {
    this.text = "";
  }

  append(line: string): void {
    this.text += `${line}\n`;
  }
}
