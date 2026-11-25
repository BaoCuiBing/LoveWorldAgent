<script setup>
import { cn } from '@/components/shadcn_ui/lib/utils'
const props = defineProps({
    class: { type: [String, Array, Object], default: '' },
})
</script>

<template>
    <h5 :class="cn('mb-0.5 font-medium leading-none tracking-tight', props.class)">
        <slot />
    </h5>
</template>
