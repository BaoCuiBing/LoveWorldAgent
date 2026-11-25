<script setup>
import { DropdownMenuRoot } from 'reka-ui'
const props = defineProps({
    open: { type: Boolean, default: undefined },
    defaultOpen: Boolean,
    modal: { type: Boolean, default: true },
})
const emits = defineEmits(['update:open'])
</script>

<template>
    <DropdownMenuRoot
        :open="props.open"
        :default-open="props.defaultOpen"
        :modal="props.modal"
        @update:open="emits('update:open', $event)"
    >
        <slot />
    </DropdownMenuRoot>
</template>
