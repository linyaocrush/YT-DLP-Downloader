<script lang="ts">
  import type { AudioSourceDto, VideoSourceDto } from "../types";

  let {
    kind,
    title,
    sources,
    selectedFormatId,
    onselect,
  }: {
    kind: "video" | "audio";
    title: string;
    sources: VideoSourceDto[] | AudioSourceDto[];
    selectedFormatId: string | null;
    onselect: (source: VideoSourceDto | AudioSourceDto) => void;
  } = $props();
</script>

<div class="card source-card">
  <div class="semi source-title">{title}</div>
  <div class="table-scroll">
    <table class="data">
      <thead>
        {#if kind === "video"}
          <tr><th>格式 ID</th><th>分辨率</th><th>帧率</th><th>码率</th><th>视频编码</th><th>类型</th><th>容器</th></tr>
        {:else}
          <tr><th>格式 ID</th><th>码率</th><th>音频编码</th><th>容器</th></tr>
        {/if}
      </thead>
      <tbody>
        {#each sources as source (source.formatId)}
          <tr
            class:selected={selectedFormatId === source.formatId}
            tabindex="0"
            aria-selected={selectedFormatId === source.formatId}
            onclick={() => onselect(source)}
            onkeydown={(event) => {
              if (event.key === "Enter" || event.key === " ") {
                event.preventDefault();
                onselect(source);
              }
            }}
          >
            {#if kind === "video"}
              {@const video = source as VideoSourceDto}
              <td>{video.formatId}</td><td>{video.resolution}</td><td>{video.fpsText}</td><td>{video.bitrateText}</td><td>{video.codec}</td><td>{video.kind}</td><td>{video.container}</td>
            {:else}
              {@const audio = source as AudioSourceDto}
              <td>{audio.formatId}</td><td>{audio.bitrateText}</td><td>{audio.codec}</td><td>{audio.container}</td>
            {/if}
          </tr>
        {/each}
      </tbody>
    </table>
  </div>
</div>
