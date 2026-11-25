<script setup>
import { cn } from '@/components/shadcn_ui/lib/utils'
const props = defineProps({
    class: { type: [String, Array, Object], default: '' },
})
</script>

<template>
    <div :class="cn('max-h-72 overflow-y-auto overflow-x-hidden p-1', props.class)">
        <slot />
    </div>
</template>
