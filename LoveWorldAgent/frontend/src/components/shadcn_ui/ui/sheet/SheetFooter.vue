<script setup>
import { cn } from '@/components/shadcn_ui/lib/utils'
const props = defineProps({
    class: { type: [String, Array, Object], default: '' },
})
</script>

<template>
    <div :class="cn('mt-auto flex flex-row justify-end gap-2', props.class)">
        <slot />
    </div>
</template>
