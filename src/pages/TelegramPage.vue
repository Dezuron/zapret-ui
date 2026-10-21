<script setup lang="ts">
import { Send } from "lucide-vue-next";
</script>

<template>
  <div class="page-stack">
    <section class="glass-card telegram-empty">
      <span class="telegram-icon"><Send :size="28" /></span>
      <span class="eyebrow">Скоро</span>
      <h2>Telegram WS-прокси</h2>
      <p class="muted">
        Здесь появится управление встроенным Telegram WS-прокси: подключение, ключ, порт и
        собственный Cloudflare Worker. Сейчас приложение сфокусировано на flowseal-zapret.
      </p>
    </section>
  </div>
</template>
