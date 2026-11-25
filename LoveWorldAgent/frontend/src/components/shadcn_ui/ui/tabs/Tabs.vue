<script setup>
import { TabsRoot } from 'reka-ui'
const props = defineProps({
    modelValue: { type: [String, Number], default: undefined },
    defaultValue: { type: [String, Number], default: undefined },
    orientation: { type: String, default: 'horizontal' },
})
const emits = defineEmits(['update:modelValue'])
</script>

<template>
    <TabsRoot
        :model-value="props.modelValue"
        :default-value="props.defaultValue"
        :orientation="props.orientation"
        @update:model-value="emits('update:modelValue', $event)"
    >
        <slot />
    </TabsRoot>
</template>
