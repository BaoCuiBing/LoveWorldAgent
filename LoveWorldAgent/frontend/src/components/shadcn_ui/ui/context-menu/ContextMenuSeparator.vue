<script setup>
import { ContextMenuSeparator } from 'reka-ui'
import { cn } from '@/components/shadcn_ui/lib/utils'
const props = defineProps({
    class: { type: [String, Array, Object], default: '' },
})
</script>

<template>
    <ContextMenuSeparator :class="cn('-mx-1 my-1 h-px bg-border', props.class)" />
</template>
