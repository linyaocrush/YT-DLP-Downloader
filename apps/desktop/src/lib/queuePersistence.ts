export class QueuePersistence<T> {
  private timer: ReturnType<typeof setTimeout> | null = null;
  private pending: T | null = null;
  private chain: Promise<void> = Promise.resolve();

  constructor(
    private readonly save: (snapshot: T) => Promise<void>,
    private readonly delayMs = 350,
    private readonly onError: (error: unknown) => void = () => {},
  ) {}

  schedule(snapshot: T): void {
    this.pending = snapshot;
    if (this.timer) clearTimeout(this.timer);
    this.timer = setTimeout(() => {
      void this.flush().catch(error => this.onError(error));
    }, this.delayMs);
  }

  async flush(): Promise<void> {
    if (this.timer) clearTimeout(this.timer);
    this.timer = null;
    while (this.pending !== null) {
      const snapshot = this.pending;
      this.pending = null;
      this.chain = this.chain.catch(() => {}).then(() => this.save(snapshot));
      try {
        await this.chain;
      } catch (error) {
        if (this.pending === null) this.pending = snapshot;
        this.onError(error);
        throw error;
      }
    }
  }
}
