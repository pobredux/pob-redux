<script lang="ts">
  let { locale, size = 12 }: { locale: string; size?: number } = $props();

  const VIEWBOX: Record<string, string> = {
    en: "0 0 640 480",
    de: "0 0 640 480",
    es: "0 0 640 480",
    fr: "0 0 640 480",
    ja: "0 0 640 480",
    ko: "0 0 36 24",
    "pt-BR": "0 0 20 14",
    ru: "0 0 640 480",
    th: "0 0 640 480",
    "zh-CN": "0 0 30 20",
    "zh-TW": "0 0 36 24",
  };
</script>

<!-- The UK, German, French, Russian and Thai flags come from flag-icons (MIT, Panayiotis Lipiridis). -->
{#if VIEWBOX[locale]}
  <svg viewBox={VIEWBOX[locale]} width={Math.round((size * 4) / 3)} height={size} preserveAspectRatio="xMidYMid slice" aria-hidden="true" class="flag">
    {#if locale === "en"}
      <path fill="#012169" d="M0 0h640v480H0z" />
      <path fill="#fff" d="m75 0 244 181L562 0h78v62L400 241l240 178v61h-80L320 301 81 480H0v-60l239-178L0 64V0z" />
      <path fill="#c8102e" d="m424 281 216 159v40L369 281zm-184 20 6 35L54 480H0zM640 0v3L391 191l2-44L590 0zM0 0l239 176h-60L0 42z" />
      <path fill="#fff" d="M241 0v480h160V0zM0 160v160h640V160z" />
      <path fill="#c8102e" d="M0 193v96h640v-96zM273 0v480h96V0z" />
    {:else if locale === "de"}
      <path fill="#000" d="M0 0h640v160H0z" />
      <path fill="#d00" d="M0 160h640v160H0z" />
      <path fill="#fc0" d="M0 320h640v160H0z" />
    {:else if locale === "es"}
      <path fill="#ad1519" d="M0 0h640v480H0z" />
      <path fill="#fabd00" d="M0 120h640v240H0z" />
    {:else if locale === "fr"}
      <path fill="#fff" d="M0 0h640v480H0z" />
      <path fill="#000091" d="M0 0h213.3v480H0z" />
      <path fill="#e1000f" d="M426.7 0H640v480H426.7z" />
    {:else if locale === "ja"}
      <path fill="#fff" d="M0 0h640v480H0z" />
      <circle cx="320" cy="240" r="144" fill="#bc002d" />
    {:else if locale === "ko"}
      <path fill="#fff" d="M0 0h36v24H0z" />
      <g transform="translate(18 12) rotate(33.69)">
        <circle r="6" fill="#0047a0" />
        <path fill="#cd2e3a" d="M-6 0A6 6 0 0 1 6 0A3 3 0 0 0 0 0A3 3 0 0 1-6 0Z" />
      </g>
      <g fill="#000">
        <path transform="translate(8.847 5.898) rotate(-236.31)" d="M-3-2h6v1h-6ZM-3-.5h6v1h-6ZM-3 1h6v1h-6Z" />
        <path transform="translate(27.153 5.898) rotate(-123.69)" d="M-3-2h2.75v1h-2.75ZM.25-2h2.75v1h-2.75ZM-3-.5h6v1h-6ZM-3 1h2.75v1h-2.75ZM.25 1h2.75v1h-2.75Z" />
        <path transform="translate(8.847 18.102) rotate(56.31)" d="M-3-2h6v1h-6ZM-3-.5h2.75v1h-2.75ZM.25-.5h2.75v1h-2.75ZM-3 1h6v1h-6Z" />
        <path transform="translate(27.153 18.102) rotate(-56.31)" d="M-3-2h2.75v1h-2.75ZM.25-2h2.75v1h-2.75ZM-3-.5h2.75v1h-2.75ZM.25-.5h2.75v1h-2.75ZM-3 1h2.75v1h-2.75ZM.25 1h2.75v1h-2.75Z" />
      </g>
    {:else if locale === "pt-BR"}
      <path fill="#009c3b" d="M0 0h20v14H0z" />
      <path fill="#ffdf00" d="M1.7 7 10 1.7 18.3 7 10 12.3Z" />
      <circle cx="10" cy="7" r="3.5" fill="#002776" />
      <path fill="none" stroke="#fff" stroke-width=".6" d="M6.687 6.235A8 8 0 0 1 13.313 7.765" />
    {:else if locale === "ru"}
      <path fill="#fff" d="M0 0h640v160H0z" />
      <path fill="#0039a6" d="M0 160h640v160H0z" />
      <path fill="#d52b1e" d="M0 320h640v160H0z" />
    {:else if locale === "th"}
      <path fill="#f4f5f8" d="M0 0h640v480H0z" />
      <path fill="#2d2a4a" d="M0 162.5h640v160H0z" />
      <path fill="#a51931" d="M0 0h640v82.5H0zm0 400h640v80H0z" />
    {:else if locale === "zh-CN"}
      <path fill="#ee1c25" d="M0 0h30v20H0z" />
      <path
        fill="#ff0"
        d="M5 2L5.674 4.073L7.853 4.073L6.09 5.354L6.763 7.427L5 6.146L3.237 7.427L3.91 5.354L2.147 4.073L4.326 4.073ZM9.143 2.514L9.62 1.966L9.246 1.343L9.914 1.628L10.391 1.08L10.328 1.803L10.996 2.088L10.288 2.251L10.224 2.975L9.851 2.352ZM11.01 4.141L11.662 3.821L11.56 3.102L12.065 3.624L12.718 3.304L12.378 3.946L12.884 4.467L12.168 4.343L11.829 4.985L11.726 4.266ZM11.038 6.725L11.765 6.699L11.964 6.001L12.213 6.683L12.939 6.657L12.367 7.105L12.616 7.787L12.014 7.382L11.442 7.83L11.641 7.131ZM9.219 8.375L9.899 8.632L10.353 8.064L10.319 8.79L10.999 9.046L10.298 9.239L10.265 9.964L9.865 9.357L9.165 9.55L9.618 8.982Z"
      />
    {:else if locale === "zh-TW"}
      <path fill="#fe0000" d="M0 0h36v24H0z" />
      <path fill="#000095" d="M0 0h18v12H0z" />
      <path
        fill="#fff"
        d="M9 1.5L9.641 3.609L11.25 2.103L10.75 4.25L12.897 3.75L11.391 5.359L13.5 6L11.391 6.641L12.897 8.25L10.75 7.75L11.25 9.897L9.641 8.391L9 10.5L8.359 8.391L6.75 9.897L7.25 7.75L5.103 8.25L6.609 6.641L4.5 6L6.609 5.359L5.103 3.75L7.25 4.25L6.75 2.103L8.359 3.609Z"
      />
      <circle cx="9" cy="6" r="2.555" fill="#000095" />
      <circle cx="9" cy="6" r="2.255" fill="#fff" />
    {/if}
  </svg>
{/if}

<style>
  .flag {
    display: block;
    flex: none;
    border-radius: 2px;
    box-shadow: 0 0 0 1px var(--line-1);
  }
</style>
