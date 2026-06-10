// Contextful · ECS Fargate demo deploy of the `sync` server binary.
//
// Builds the repo-root Dockerfile (linux/arm64 → Fargate Graviton), pushes to
// ECR, and runs one task in the account's default VPC behind a network load
// balancer: TCP 7878 (Loro WS relay) and TCP 7979 (brain MCP streamable HTTP).
//
//   cd infra/ecs && pnpm install && pnpm up
//
// Credentials come from dotenvx (.env.production, decrypted via .env.keys).
// Demo-account posture on purpose: ports are world-reachable, state is
// ephemeral inside the task. The trust-boundary note in ../index.ts does not
// apply here — this runs the seeded demo scenario, no real source data.

import * as pulumi from "@pulumi/pulumi";
import * as aws from "@pulumi/aws";
import * as awsx from "@pulumi/awsx";

const cfg = new pulumi.Config("contextful-ecs");
const syncPort = cfg.getNumber("syncPort") ?? 7878;
const mcpPort = cfg.getNumber("mcpPort") ?? 7979;

// --- Image: repo-root Dockerfile → ECR -------------------------------------

const repo = new awsx.ecr.Repository("sync", {
  forceDelete: true, // demo: let `pulumi destroy` remove a non-empty repo
});

const image = new awsx.ecr.Image("sync", {
  repositoryUrl: repo.url,
  context: "../..",
  dockerfile: "../../Dockerfile",
  platform: "linux/arm64",
});

// --- Network: default VPC, world-open demo security group ------------------

const vpc = aws.ec2.getVpcOutput({ default: true });
const subnets = aws.ec2.getSubnetsOutput({
  filters: [{ name: "vpc-id", values: [vpc.id] }],
});

const sg = new aws.ec2.SecurityGroup("sync", {
  vpcId: vpc.id,
  description: "Contextful demo relay + MCP (world-open by design)",
  ingress: [syncPort, mcpPort].map((port) => ({
    protocol: "tcp",
    fromPort: port,
    toPort: port,
    cidrBlocks: ["0.0.0.0/0"],
  })),
  egress: [
    { protocol: "-1", fromPort: 0, toPort: 0, cidrBlocks: ["0.0.0.0/0"] },
  ],
});

const nlb = new aws.lb.LoadBalancer("sync", {
  loadBalancerType: "network",
  subnets: subnets.ids,
});

const makeTarget = (name: string, port: number) => {
  const tg = new aws.lb.TargetGroup(name, {
    port,
    protocol: "TCP",
    targetType: "ip",
    vpcId: vpc.id,
    deregistrationDelay: 30,
  });
  const listener = new aws.lb.Listener(name, {
    loadBalancerArn: nlb.arn,
    port,
    protocol: "TCP",
    defaultActions: [{ type: "forward", targetGroupArn: tg.arn }],
  });
  return { tg, listener };
};

const relay = makeTarget("sync-relay", syncPort);
const mcp = makeTarget("sync-mcp", mcpPort);

// --- ECS: cluster, task definition, service --------------------------------

const cluster = new aws.ecs.Cluster("sync");

const logGroup = new aws.cloudwatch.LogGroup("sync", { retentionInDays: 7 });

const execRole = new aws.iam.Role("sync-exec", {
  assumeRolePolicy: JSON.stringify({
    Version: "2012-10-17",
    Statement: [
      {
        Effect: "Allow",
        Principal: { Service: "ecs-tasks.amazonaws.com" },
        Action: "sts:AssumeRole",
      },
    ],
  }),
});
new aws.iam.RolePolicyAttachment("sync-exec", {
  role: execRole.name,
  policyArn:
    "arn:aws:iam::aws:policy/service-role/AmazonECSTaskExecutionRolePolicy",
});

const region = aws.getRegionOutput().name;

const taskDefinition = new aws.ecs.TaskDefinition("sync", {
  family: "contextful-sync",
  cpu: "512",
  memory: "1024",
  networkMode: "awsvpc",
  requiresCompatibilities: ["FARGATE"],
  runtimePlatform: { cpuArchitecture: "ARM64", operatingSystemFamily: "LINUX" },
  executionRoleArn: execRole.arn,
  containerDefinitions: pulumi.jsonStringify([
    {
      name: "sync",
      image: image.imageUri,
      essential: true,
      portMappings: [
        { containerPort: syncPort, protocol: "tcp" },
        { containerPort: mcpPort, protocol: "tcp" },
      ],
      environment: [{ name: "RUST_LOG", value: "info" }],
      logConfiguration: {
        logDriver: "awslogs",
        options: {
          "awslogs-group": logGroup.name,
          "awslogs-region": region,
          "awslogs-stream-prefix": "sync",
        },
      },
    },
  ]),
});

new aws.ecs.Service(
  "sync",
  {
    cluster: cluster.arn,
    desiredCount: 1,
    launchType: "FARGATE",
    taskDefinition: taskDefinition.arn,
    networkConfiguration: {
      subnets: subnets.ids,
      securityGroups: [sg.id],
      assignPublicIp: true, // default VPC public subnets; needed to pull from ECR
    },
    loadBalancers: [
      { targetGroupArn: relay.tg.arn, containerName: "sync", containerPort: syncPort },
      { targetGroupArn: mcp.tg.arn, containerName: "sync", containerPort: mcpPort },
    ],
  },
  { dependsOn: [relay.listener, mcp.listener] },
);

export const imageUri = image.imageUri;
export const nlbDnsName = nlb.dnsName;
export const relayUrl = pulumi.interpolate`ws://${nlb.dnsName}:${syncPort}`;
export const mcpUrl = pulumi.interpolate`http://${nlb.dnsName}:${mcpPort}/mcp`;
