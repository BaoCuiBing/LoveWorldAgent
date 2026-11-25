<script setup>
import { SelectGroup } from 'reka-ui'
import { cn } from '@/components/shadcn_ui/lib/utils'
const props = defineProps({
    class: { type: [String, Array, Object], default: '' },
})
</script>

<template>
    <SelectGroup :class="cn('p-0', props.class)">
        <slot />
    </SelectGroup>
</template>
