<script setup lang="ts">
import { ref } from "vue";
import CmpButton from "../components/CmpButton.vue";
import CmpInput from "../components/CmpInput.vue";
import CmpCard from "../components/CmpCard.vue";
import CmpSwitch from "../components/CmpSwitch.vue";
import CmpTag from "../components/CmpTag.vue";
import catalog from "../mcp-meta/catalog.json";

/** Homepage built from CompiraMCP tools:
 *  list_libraries → search_components → get_component → get_component_source
 *  Components below are the exact SFCs returned by get_component_source.
 */

const email = ref("");
const notify = ref(true);
const tags = ref(["MCP 导出", "Vue 3", "真实 Props"]);
const clicks = ref(0);
const submitted = ref(false);

function removeTag(i: number) {
  tags.value = tags.value.filter((_, idx) => idx !== i);
}

function onSubmit() {
  clicks.value += 1;
  submitted.value = true;
}
</script>

<template>
  <div class="page">
    <div class="grain" aria-hidden="true" />

    <header class="top">
      <div class="brand">Compira</div>
      <p class="eyebrow">MCP 组件实装验证 · Demo UI 库</p>
    </header>

    <section class="hero">
      <h1>
        用 MCP 拿到的组件<br />
        <em>直接铺开首页</em>
      </h1>
      <p class="lede">
        本页全部交互控件来自 CompiraMCP：
        <code>get_component_source</code> 导出的
        <strong>CmpButton / CmpInput / CmpCard / CmpSwitch / CmpTag</strong>。
        规则校验：<code>prefer:Cmp</code> 已通过
        <code>validate_code</code>。
      </p>

      <div class="hero-actions">
        <CmpButton type="primary" size="large" label="开始试用" @click="onSubmit" />
        <CmpButton type="default" size="large" label="查看源码" />
      </div>

      <div class="tag-row">
        <CmpTag
          v-for="(t, i) in tags"
          :key="t"
          type="primary"
          closable
          @close="removeTag(i)"
        >
          {{ t }}
        </CmpTag>
        <CmpTag type="success">validate ✓</CmpTag>
      </div>
    </section>

    <section class="grid">
      <CmpCard title="订阅更新" shadow="hover" class="panel">
        <p class="panel-copy">用 MCP 元数据里的 Props：placeholder / clearable / v-model。</p>
        <div class="field">
          <CmpInput v-model="email" placeholder="you@studio.dev" clearable />
        </div>
        <div class="field row">
          <CmpSwitch
            v-model="notify"
            active-text="邮件提醒开"
            inactive-text="邮件提醒关"
          />
        </div>
        <template #footer>
          <CmpButton
            type="success"
            :label="submitted ? `已提交 ×${clicks}` : '提交'"
            :disabled="!email"
            @click="onSubmit"
          />
        </template>
      </CmpCard>

      <CmpCard title="MCP 目录" shadow="always" class="panel">
        <ul class="catalog">
          <li v-for="c in catalog.components" :key="c.id">
            <span class="cat-name">{{ c.name }}</span>
            <span class="cat-meta">
              {{ c.props.length }} props · {{ c.slots.length }} slots
            </span>
            <span class="cat-desc">{{ c.description || "—" }}</span>
          </li>
        </ul>
        <template #footer>
          <span class="foot-note">
            library: {{ catalog.rules.name }} · rules: {{ catalog.rules.rules }}
          </span>
        </template>
      </CmpCard>

      <CmpCard title="交互回声" shadow="hover" class="panel">
        <dl class="echo">
          <div>
            <dt>email</dt>
            <dd>{{ email || "（空）" }}</dd>
          </div>
          <div>
            <dt>notify</dt>
            <dd>{{ notify ? "on" : "off" }}</dd>
          </div>
          <div>
            <dt>clicks</dt>
            <dd>{{ clicks }}</dd>
          </div>
        </dl>
        <div class="btn-row">
          <CmpButton type="warning" size="small" label="警告" />
          <CmpButton type="danger" size="small" label="危险" />
          <CmpButton type="primary" size="small" label="主色" disabled />
        </div>
      </CmpCard>
    </section>

    <footer class="foot">
      <p>
        组件源码路径：
        <code>mcp-demo-home/src/components/*.vue</code>
        ← 由
        <code>scripts/mcp-export-components.py</code>
        调用 MCP 工具写入。
      </p>
    </footer>
  </div>
</template>
