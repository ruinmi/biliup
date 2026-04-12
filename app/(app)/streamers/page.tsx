'use client'
import {
  Layout,
  Nav,
  Button,
  Tag,
  Typography,
  Popconfirm,
  Notification,
  Card,
  Dropdown,
  Badge,
} from '@douyinfe/semi-ui'
import {
  IconHelpCircle,
  IconPlusCircle,
  IconVideoListStroked,
  IconEdit2Stroked,
  IconDeleteStroked,
  IconWrench,
  IconTreeTriangleDown,
  IconPause,
  IconPlay,
  IconLock,
  IconUpload,
} from '@douyinfe/semi-icons'
import { List, ButtonGroup } from '@douyinfe/semi-ui'
import React, { useState } from 'react'
import useStreamers from '../../lib/use-streamers'
import TemplateModal from '../../ui/TemplateModal'
import OverrideModal from '../../ui/OverrideModal'
import {
  HookFormCommand,
  HookFormStep,
  LiveStreamerEntity,
  put,
  requestDelete,
  sendRequest,
} from '../../lib/api-streamer'
import useSWRMutation from 'swr/mutation'
import { PauseButton } from '@/app/ui/StreamerActions/PauseButton'

type HookUiStep =
  | HookFormStep
  | 'rm'
  | {
      run: string
    }
  | {
      mv: string
    }
  | {
      webhook: string
    }

const isHookFormCommand = (value: unknown): value is HookFormCommand =>
  value === 'run' || value === 'mv' || value === 'rm' || value === 'webhook'

const normalizeHookSteps = (steps?: HookUiStep[]): HookUiStep[] | undefined =>
  steps?.map(step => {
    if (step === 'rm') {
      return { cmd: 'rm' as const }
    }

    if (!step || typeof step !== 'object') {
      return step
    }

    const formStep = step as HookFormStep
    if (isHookFormCommand(formStep.cmd)) {
      return formStep
    }

    const [cmd, value] = Object.entries(step)[0] ?? []
    if (!isHookFormCommand(cmd)) {
      return step
    }

    return value === undefined ? { cmd } : { cmd, value: String(value) }
  })

const serializeHookSteps = (steps?: HookUiStep[]): HookUiStep[] | undefined =>
  steps?.map(step => {
    if (!step || typeof step !== 'object') {
      return step
    }

    const formStep = step as HookFormStep
    if (!isHookFormCommand(formStep.cmd)) {
      return step
    }

    if (formStep.cmd === 'rm') {
      return 'rm'
    }

    return { [formStep.cmd]: formStep.value ?? '' } as HookUiStep
  })

const normalizeHookFields = (values: LiveStreamerEntity): LiveStreamerEntity => ({
  ...values,
  downloaded_processor: normalizeHookSteps(values.downloaded_processor as HookUiStep[] | undefined),
  postprocessor: normalizeHookSteps(values.postprocessor as HookUiStep[] | undefined),
})

const serializeHookFields = (values: any) => ({
  ...values,
  downloaded_processor: serializeHookSteps(
    values?.downloaded_processor as HookUiStep[] | undefined
  ),
  postprocessor: serializeHookSteps(values?.postprocessor as HookUiStep[] | undefined),
})

export default function Home() {
  const { Header, Content } = Layout
  const { Text } = Typography
  const { streamers, isLoading } = useStreamers()
  const { trigger: deleteStreamers } = useSWRMutation('/v1/streamers', requestDelete)
  const { trigger: updateStreamers } = useSWRMutation('/v1/streamers', put)
  const { trigger } = useSWRMutation('/v1/streamers', sendRequest)

  const onConfirm = async (id: number) => {
    await deleteStreamers(id)
  }
  const data: LiveStreamerEntity[] | undefined = streamers?.map(live => {
    let statusTag
    switch (live.status) {
      case 'Working':
        statusTag = <Tag color="red">直播中</Tag>
        break
      case 'Idle':
        statusTag = <Tag color="green">空闲</Tag>
        break
      case 'Pending':
        statusTag = <Tag color="indigo">检测中</Tag>
        break
      case 'OutOfSchedule':
      case 'OutOfSchedule:TimeRange':
        statusTag = <Tag color="yellow">非录播时间</Tag>
        break
      case 'OutOfSchedule:ExcludedKeywords':
        statusTag = <Tag color="yellow">标题被排除</Tag>
        break
      case 'Pause':
        statusTag = <Tag color="pink">暂停中</Tag>
        break
    }
    return { ...normalizeHookFields(live), statusTag }
  })

  const handleOk = async (values: any) => {
    values = serializeHookFields(values)
    try {
      const res = await trigger(values)
    } catch (e: any) {
      Notification.error({
        title: '创建失败',
        content: <Typography.Paragraph style={{ maxWidth: 450 }}>{e.message}</Typography.Paragraph>,
        style: { width: 'min-content' },
      })
      throw e
    }
  }

  const handleUpdate = async (values: any) => {
    console.log(values)
    delete values.status
    delete values.statusTag
    delete values.upload_status
    values = serializeHookFields(values)
    try {
      const res = await updateStreamers(values)
    } catch (e: any) {
      Notification.error({
        title: '更新失败',
        content: <Typography.Paragraph style={{ maxWidth: 450 }}>{e.message}</Typography.Paragraph>,
        style: { width: 'min-content' },
      })
      throw e
    }
  }

  return (
    <>
      <Header
        style={{
          backgroundColor: 'var(--semi-color-bg-1)',
          position: 'sticky',
          top: 0,
          zIndex: 1,
        }}
      >
        <nav
          style={{
            display: 'flex',
            paddingLeft: '25px',
            paddingRight: '25px',
            alignItems: 'center',
            justifyContent: 'space-between',
            flexWrap: 'wrap',
            boxShadow: '0 1px 2px 0 rgb(0 0 0 / 0.05)',
          }}
        >
          <div
            style={{
              display: 'flex',
              gap: 10,
              justifyContent: 'center',
              alignItems: 'center',
              flexWrap: 'wrap',
            }}
          >
            <IconVideoListStroked
              size="large"
              style={{
                backgroundColor: 'rgba(var(--semi-green-4), 1)',
                borderRadius: 'var(--semi-border-radius-large)',
                color: 'var(--semi-color-bg-0)',
                padding: '6px',
              }}
            />
            <h4>录播管理</h4>
          </div>
          <div
            style={{
              display: 'flex',
              flexWrap: 'wrap',
              alignItems: 'center',
              justifyContent: 'center',
              gap: 6,
            }}
          >
            <Button
              theme="borderless"
              icon={<IconHelpCircle size="large" />}
              style={{
                color: 'var(--semi-color-text-2)',
              }}
              onClick={() => (window.location.href = '/static/ds_update.log')}
            />
            <TemplateModal onOk={handleOk}>
              <Button icon={<IconPlusCircle />} theme="solid" style={{ marginRight: 10 }}>
                新建
              </Button>
            </TemplateModal>
          </div>
        </nav>
      </Header>
      <Content
        style={{
          padding: '24px',
          backgroundColor: 'var(--semi-color-bg-0)',
        }}
      >
        <main>
          <List
            grid={{
              gutter: 12,
              xs: 24,
              sm: 24,
              md: 12,
              lg: 8,
              xl: 6,
              xxl: 4,
            }}
            dataSource={data}
            renderItem={item => (
              <List.Item>
                <Card
                  shadows="hover"
                  style={{
                    // maxWidth: 360,
                    margin: '9px 0px',
                    width: '100%',
                    // flexGrow: 1,
                  }}
                  bodyStyle={
                    {
                      // display: 'flex',
                      // alignItems: 'center',
                      // justifyContent: 'space-between'
                    }
                  }
                >
                  <div style={{ position: 'absolute', right: 20, top: 9 }}>{item.statusTag}</div>

                  <div style={{ display: 'flex', justifyContent: 'space-between' }}>
                    {item.upload_status === 'Pending' ? (
                      <Badge count={<IconUpload />}> </Badge>
                    ) : null}

                    <h3
                      style={{
                        color: 'var(--semi-color-text-0)',
                        fontWeight: 500,
                        maxWidth: '80%',
                        overflow: 'hidden',
                        textOverflow: 'ellipsis',
                        whiteSpace: 'nowrap',
                      }}
                    >
                      {item.remark}
                    </h3>
                  </div>

                  <Text style={{ width: '101%' }} ellipsis={{ showTooltip: true }} type="tertiary">
                    {item.url}
                  </Text>

                  <div
                    style={{
                      margin: '0',
                      display: 'flex',
                      padding: '0 0 32px 0px',
                      justifyContent: 'flex-end',
                    }}
                  >
                    <ButtonGroup
                      theme="borderless"
                      style={{ position: 'absolute', right: 20, bottom: 15 }}
                    >
                      <TemplateModal onOk={handleUpdate} entity={item}>
                        <Button theme="borderless" icon={<IconEdit2Stroked />}></Button>
                      </TemplateModal>
                      <span className="semi-button-group-line semi-button-group-line-borderless semi-button-group-line-primary"></span>
                      <PauseButton streamer={item} />
                      <span className="semi-button-group-line semi-button-group-line-borderless semi-button-group-line-primary"></span>
                      <Popconfirm
                        title="确定是否要删除？"
                        content="此操作将不可逆"
                        onConfirm={async () => await onConfirm(item.id)}
                        // onCancel={onCancel}
                      >
                        <Button theme="borderless" icon={<IconDeleteStroked />}></Button>
                      </Popconfirm>
                      <span className="semi-button-group-line semi-button-group-line-borderless semi-button-group-line-primary"></span>
                      <OverrideModal onOk={handleUpdate} entity={item}>
                        <Button theme="borderless" icon={<IconWrench />}></Button>
                      </OverrideModal>
                    </ButtonGroup>
                  </div>
                </Card>
              </List.Item>
            )}
          />
        </main>
      </Content>
    </>
  )
}
